use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::Duration;

use anyhow::anyhow;
use anyhow::Context;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use wasmtime::component::{Resource, ResourceTable};
use wasmtime_wasi::filesystem::WasiFilesystemView as _;
use wasmtime_wasi::p2::bindings::sync::io::{poll, streams};
use wasmtime_wasi::p2::bindings::{cli, clocks, random, sync::filesystem};
use wasmtime_wasi::p2::{FsError, FsResult, StreamError, StreamResult};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::types::{
    HostFutureIncomingResponse, IncomingResponse, OutgoingRequestConfig,
};
use wasmtime_wasi_http::{HttpError, WasiHttpCtx, WasiHttpView};

use crate::trace::{TraceEvent, TraceFile, TraceFormat};
use crate::util::cbor::is_cbor_eof;
use crate::wasi::util::{header_map_from_pairs, sorted_headers};
use anyhow::Result;

enum PlaybackSource {
    /// All events loaded in memory (used for JSON traces)
    Memory(VecDeque<TraceEvent>),
    /// Streaming from a CBOR file
    Stream(BufReader<File>),
}

pub struct Playback {
    source: PlaybackSource,
}

impl Playback {
    pub fn from_file(path: &Path, format: TraceFormat) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("failed to open trace file at {}", path.display()))?;
        let reader = BufReader::new(file);

        let source = match format {
            TraceFormat::Json => {
                let TraceFile { events } = serde_json::from_reader(reader).with_context(|| {
                    format!("failed to parse JSON trace file at {}", path.display())
                })?;
                PlaybackSource::Memory(events.into())
            }
            TraceFormat::Cbor => {
                // For CBOR, we stream events on demand instead of loading all at once
                PlaybackSource::Stream(reader)
            }
        };

        Ok(Self { source })
    }

    pub fn next_event(&mut self) -> Result<TraceEvent> {
        match &mut self.source {
            PlaybackSource::Memory(events) => events.pop_front().ok_or(anyhow!("trace exhausted")),
            PlaybackSource::Stream(reader) => {
                match ciborium::from_reader::<TraceEvent, _>(&mut *reader) {
                    Ok(event) => Ok(event),
                    Err(e) if is_cbor_eof(&e) => Err(anyhow!("trace exhausted")),
                    Err(e) => Err(anyhow::Error::msg(format!("{}", e)))
                        .context("failed to read next event from CBOR trace"),
                }
            }
        }
    }

    pub fn next_now(&mut self) -> Result<clocks::wall_clock::Datetime> {
        match self.next_event()? {
            TraceEvent::ClockNow {
                seconds,
                nanoseconds,
            } => Ok(clocks::wall_clock::Datetime {
                seconds,
                nanoseconds,
            }),
            other => Err(anyhow!(
                "expected next clock event to be 'now', got {:?}",
                other
            )),
        }
    }

    pub fn next_resolution(&mut self) -> Result<clocks::wall_clock::Datetime> {
        match self.next_event()? {
            TraceEvent::ClockResolution {
                seconds,
                nanoseconds,
            } => Ok(clocks::wall_clock::Datetime {
                seconds,
                nanoseconds,
            }),
            other => Err(anyhow!(
                "expected next clock event to be 'resolution', got {:?}",
                other
            )),
        }
    }

    pub fn next_monotonic_now(&mut self) -> Result<u64> {
        match self.next_event()? {
            TraceEvent::MonotonicClockNow { nanoseconds } => Ok(nanoseconds),
            other => Err(anyhow!(
                "expected next monotonic clock event to be 'now', got {:?}",
                other
            )),
        }
    }

    pub fn next_monotonic_resolution(&mut self) -> Result<u64> {
        match self.next_event()? {
            TraceEvent::MonotonicClockResolution { nanoseconds } => Ok(nanoseconds),
            other => Err(anyhow!(
                "expected next monotonic clock event to be 'resolution', got {:?}",
                other
            )),
        }
    }

    pub fn next_environment(&mut self) -> Result<Vec<(String, String)>> {
        match self.next_event()? {
            TraceEvent::Environment { entries } => Ok(entries),
            other => Err(anyhow!("expected next environment event, got {:?}", other)),
        }
    }

    pub fn next_arguments(&mut self) -> Result<Vec<String>> {
        match self.next_event()? {
            TraceEvent::Arguments { args } => Ok(args),
            other => Err(anyhow!("expected next arguments event, got {:?}", other)),
        }
    }

    pub fn next_initial_cwd(&mut self) -> Result<Option<String>> {
        match self.next_event()? {
            TraceEvent::InitialCwd { path } => Ok(path),
            other => Err(anyhow!("expected next initial_cwd event, got {:?}", other)),
        }
    }

    pub fn next_random_bytes(&mut self, expected_len: u64) -> Result<Vec<u8>> {
        match self.next_event()? {
            TraceEvent::RandomBytes { bytes } => {
                if bytes.len() as u64 != expected_len {
                    return Err(anyhow!(
                        "random bytes length mismatch: expected {}, got {}",
                        expected_len,
                        bytes.len()
                    ));
                }
                Ok(bytes)
            }
            other => Err(anyhow!("expected next random_bytes event, got {:?}", other)),
        }
    }

    pub fn next_random_u64(&mut self) -> Result<u64> {
        match self.next_event()? {
            TraceEvent::RandomU64 { value } => Ok(value),
            other => Err(anyhow!("expected next random_u64 event, got {:?}", other)),
        }
    }

    pub fn next_insecure_random_bytes(&mut self, expected_len: u64) -> Result<Vec<u8>> {
        match self.next_event()? {
            TraceEvent::InsecureRandomBytes { bytes } => {
                if bytes.len() as u64 != expected_len {
                    return Err(anyhow!(
                        "insecure random bytes length mismatch: expected {}, got {}",
                        expected_len,
                        bytes.len()
                    ));
                }
                Ok(bytes)
            }
            other => Err(anyhow!(
                "expected next insecure_random_bytes event, got {:?}",
                other
            )),
        }
    }

    pub fn next_insecure_random_u64(&mut self) -> Result<u64> {
        match self.next_event()? {
            TraceEvent::InsecureRandomU64 { value } => Ok(value),
            other => Err(anyhow!(
                "expected next insecure_random_u64 event, got {:?}",
                other
            )),
        }
    }

    pub fn next_insecure_seed(&mut self) -> Result<(u64, u64)> {
        match self.next_event()? {
            TraceEvent::InsecureSeed { seed } => Ok(seed),
            other => Err(anyhow!(
                "expected next insecure_seed event, got {:?}",
                other
            )),
        }
    }

    pub fn expect_read_event(&mut self) -> Result<()> {
        match self.next_event() {
            Ok(TraceEvent::Read) => Ok(()),
            Ok(other) => Err(anyhow!(
                "expected next filesystem event to be 'read', got {:?}",
                other
            )),
            Err(err) => Err(err),
        }
    }

    fn next_http_response(&mut self) -> Result<(RecordedHttpRequest, RecordedHttpResponse)> {
        match self.next_event()? {
            TraceEvent::HttpResponse {
                request_method,
                request_url,
                request_headers,
                status,
                headers,
                body,
            } => Ok((
                RecordedHttpRequest {
                    method: request_method,
                    url: request_url,
                    headers: request_headers,
                },
                RecordedHttpResponse {
                    status,
                    headers,
                    body,
                },
            )),
            other => Err(anyhow!(
                "expected next http_response event, got {:?}",
                other
            )),
        }
    }

    pub fn finish(mut self) -> Result<()> {
        match &mut self.source {
            PlaybackSource::Memory(events) => {
                if events.iter().all(|event| matches!(event, TraceEvent::Read)) {
                    events.clear();
                    Ok(())
                } else {
                    Err(anyhow!(
                        "trace contains unused events: {:?}",
                        events.iter().collect::<Vec<_>>()
                    ))
                }
            }
            PlaybackSource::Stream(reader) => loop {
                match ciborium::from_reader::<TraceEvent, _>(&mut *reader) {
                    Ok(TraceEvent::Read) => continue,
                    Ok(event) => {
                        return Err(anyhow!(
                            "trace contains unused events, starting with: {:?}",
                            event
                        ))
                    }
                    Err(e) => {
                        if is_cbor_eof(&e) {
                            return Ok(());
                        }
                        return Err(anyhow::Error::msg(format!("{}", e)))
                            .context("error while checking for remaining events in CBOR trace");
                    }
                }
            },
        }
    }
}

struct RecordedHttpRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
}

struct RecordedHttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

pub struct CtxPlayback {
    table: ResourceTable,
    wasi: WasiCtx,
    http: WasiHttpCtx,
    playback: Playback,
}

impl CtxPlayback {
    pub fn new(wasi: WasiCtx, http: WasiHttpCtx, playback: Playback) -> Self {
        Self {
            table: ResourceTable::new(),
            wasi,
            http,
            playback,
        }
    }

    pub fn into_playback(self) -> Playback {
        self.playback
    }
}

impl WasiView for CtxPlayback {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for CtxPlayback {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn send_request(
        &mut self,
        request: hyper::Request<wasmtime_wasi_http::body::HyperOutgoingBody>,
        _config: OutgoingRequestConfig,
    ) -> wasmtime_wasi_http::HttpResult<HostFutureIncomingResponse> {
        let method = request.method().to_string();
        let url = request.uri().to_string();
        let actual_headers = sorted_headers(request.headers())?;

        let (expected_request, recorded_response) = self
            .playback
            .next_http_response()
            .map_err(HttpError::trap)?;

        if method != expected_request.method || url != expected_request.url {
            return Err(HttpError::trap(anyhow!(
                "http request mismatch: expected {} {}, got {method} {url}",
                expected_request.method,
                expected_request.url
            )));
        }

        if actual_headers != expected_request.headers {
            return Err(HttpError::trap(anyhow!(
                "http request headers mismatch for {method} {url}"
            )));
        }

        let RecordedHttpResponse {
            status,
            headers,
            body,
        } = recorded_response;

        let mut builder = hyper::Response::builder().status(status);
        *builder
            .headers_mut()
            .ok_or_else(|| HttpError::trap(anyhow!("failed to access response headers")))? =
            header_map_from_pairs(&headers)?;

        // Full<Bytes> is infallible, but we need to convert the error type to match the expected signature.
        // We use an explicit match on Infallible rather than Into::into because ErrorCode doesn't
        // implement From<Infallible>.
        let boxed_body = Full::new(Bytes::from(body))
            .map_err(|e: std::convert::Infallible| match e {})
            .boxed_unsync();

        let response = builder.body(boxed_body).map_err(HttpError::trap)?;

        let incoming = IncomingResponse {
            resp: response,
            worker: None,
            between_bytes_timeout: Duration::from_secs(600),
        };

        Ok(HostFutureIncomingResponse::ready(Ok(Ok(incoming))))
    }
}

impl clocks::wall_clock::Host for CtxPlayback {
    fn now(&mut self) -> std::result::Result<clocks::wall_clock::Datetime, anyhow::Error> {
        self.playback.next_now()
    }

    fn resolution(&mut self) -> std::result::Result<clocks::wall_clock::Datetime, anyhow::Error> {
        self.playback.next_resolution()
    }
}

impl clocks::monotonic_clock::Host for CtxPlayback {
    fn now(&mut self) -> anyhow::Result<u64> {
        self.playback.next_monotonic_now()
    }

    fn resolution(&mut self) -> anyhow::Result<u64> {
        self.playback.next_monotonic_resolution()
    }

    fn subscribe_instant(
        &mut self,
        when: u64,
    ) -> anyhow::Result<
        wasmtime::component::Resource<
            wasmtime_wasi::p2::bindings::clocks::monotonic_clock::Pollable,
        >,
    > {
        // Delegate to underlying WasiClocks implementation
        use wasmtime_wasi::clocks::WasiClocksView;
        self.clocks().subscribe_instant(when)
    }

    fn subscribe_duration(
        &mut self,
        duration: u64,
    ) -> anyhow::Result<
        wasmtime::component::Resource<
            wasmtime_wasi::p2::bindings::clocks::monotonic_clock::Pollable,
        >,
    > {
        // Delegate to underlying WasiClocks implementation
        use wasmtime_wasi::clocks::WasiClocksView;
        self.clocks().subscribe_duration(duration)
    }
}

impl cli::environment::Host for CtxPlayback {
    fn get_environment(&mut self) -> anyhow::Result<Vec<(String, String)>> {
        self.playback.next_environment()
    }

    fn get_arguments(&mut self) -> anyhow::Result<Vec<String>> {
        self.playback.next_arguments()
    }

    fn initial_cwd(&mut self) -> anyhow::Result<Option<String>> {
        self.playback.next_initial_cwd()
    }
}

impl random::random::Host for CtxPlayback {
    fn get_random_bytes(&mut self, len: u64) -> anyhow::Result<Vec<u8>> {
        self.playback.next_random_bytes(len)
    }

    fn get_random_u64(&mut self) -> anyhow::Result<u64> {
        self.playback.next_random_u64()
    }
}

impl random::insecure::Host for CtxPlayback {
    fn get_insecure_random_bytes(&mut self, len: u64) -> anyhow::Result<Vec<u8>> {
        self.playback.next_insecure_random_bytes(len)
    }

    fn get_insecure_random_u64(&mut self) -> anyhow::Result<u64> {
        self.playback.next_insecure_random_u64()
    }
}

impl random::insecure_seed::Host for CtxPlayback {
    fn insecure_seed(&mut self) -> anyhow::Result<(u64, u64)> {
        self.playback.next_insecure_seed()
    }
}

impl streams::Host for CtxPlayback {
    fn convert_stream_error(&mut self, err: StreamError) -> anyhow::Result<streams::StreamError> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::Host>::convert_stream_error(view.table, err)
    }
}

impl streams::HostInputStream for CtxPlayback {
    fn drop(&mut self, stream: Resource<streams::InputStream>) -> anyhow::Result<()> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostInputStream>::drop(view.table, stream)
    }

    fn read(&mut self, stream: Resource<streams::InputStream>, len: u64) -> StreamResult<Vec<u8>> {
        self.playback
            .expect_read_event()
            .map_err(|err| StreamError::trap(&err.to_string()))?;
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostInputStream>::read(view.table, stream, len)
    }

    fn blocking_read(
        &mut self,
        stream: Resource<streams::InputStream>,
        len: u64,
    ) -> StreamResult<Vec<u8>> {
        self.playback
            .expect_read_event()
            .map_err(|err| StreamError::trap(&err.to_string()))?;
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostInputStream>::blocking_read(view.table, stream, len)
    }

    fn skip(&mut self, stream: Resource<streams::InputStream>, len: u64) -> StreamResult<u64> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostInputStream>::skip(view.table, stream, len)
    }

    fn blocking_skip(
        &mut self,
        stream: Resource<streams::InputStream>,
        len: u64,
    ) -> StreamResult<u64> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostInputStream>::blocking_skip(view.table, stream, len)
    }

    fn subscribe(
        &mut self,
        stream: Resource<streams::InputStream>,
    ) -> anyhow::Result<Resource<poll::Pollable>> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostInputStream>::subscribe(view.table, stream)
    }
}

impl streams::HostOutputStream for CtxPlayback {
    fn drop(&mut self, stream: Resource<streams::OutputStream>) -> anyhow::Result<()> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostOutputStream>::drop(view.table, stream)
    }

    fn check_write(&mut self, stream: Resource<streams::OutputStream>) -> StreamResult<u64> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostOutputStream>::check_write(view.table, stream)
    }

    fn write(
        &mut self,
        stream: Resource<streams::OutputStream>,
        bytes: Vec<u8>,
    ) -> StreamResult<()> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostOutputStream>::write(view.table, stream, bytes)
    }

    fn blocking_write_and_flush(
        &mut self,
        stream: Resource<streams::OutputStream>,
        bytes: Vec<u8>,
    ) -> StreamResult<()> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostOutputStream>::blocking_write_and_flush(
            view.table, stream, bytes,
        )
    }

    fn blocking_write_zeroes_and_flush(
        &mut self,
        stream: Resource<streams::OutputStream>,
        len: u64,
    ) -> StreamResult<()> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostOutputStream>::blocking_write_zeroes_and_flush(
            view.table, stream, len,
        )
    }

    fn subscribe(
        &mut self,
        stream: Resource<streams::OutputStream>,
    ) -> anyhow::Result<Resource<poll::Pollable>> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostOutputStream>::subscribe(view.table, stream)
    }

    fn write_zeroes(
        &mut self,
        stream: Resource<streams::OutputStream>,
        len: u64,
    ) -> StreamResult<()> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostOutputStream>::write_zeroes(view.table, stream, len)
    }

    fn flush(&mut self, stream: Resource<streams::OutputStream>) -> StreamResult<()> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostOutputStream>::flush(view.table, stream)
    }

    fn blocking_flush(&mut self, stream: Resource<streams::OutputStream>) -> StreamResult<()> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostOutputStream>::blocking_flush(view.table, stream)
    }

    fn splice(
        &mut self,
        dst: Resource<streams::OutputStream>,
        src: Resource<streams::InputStream>,
        len: u64,
    ) -> StreamResult<u64> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostOutputStream>::splice(view.table, dst, src, len)
    }

    fn blocking_splice(
        &mut self,
        dst: Resource<streams::OutputStream>,
        src: Resource<streams::InputStream>,
        len: u64,
    ) -> StreamResult<u64> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostOutputStream>::blocking_splice(view.table, dst, src, len)
    }
}

impl filesystem::types::Host for CtxPlayback {
    fn convert_error_code(
        &mut self,
        err: wasmtime_wasi::p2::FsError,
    ) -> anyhow::Result<filesystem::types::ErrorCode> {
        self.filesystem().convert_error_code(err)
    }

    fn filesystem_error_code(
        &mut self,
        err: Resource<streams::Error>,
    ) -> anyhow::Result<Option<filesystem::types::ErrorCode>> {
        self.filesystem().filesystem_error_code(err)
    }
}

impl filesystem::types::HostDescriptor for CtxPlayback {
    fn advise(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        offset: filesystem::types::Filesize,
        len: filesystem::types::Filesize,
        advice: filesystem::types::Advice,
    ) -> FsResult<()> {
        self.filesystem().advise(fd, offset, len, advice)
    }

    fn sync_data(&mut self, fd: Resource<filesystem::types::Descriptor>) -> FsResult<()> {
        self.filesystem().sync_data(fd)
    }

    fn get_flags(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
    ) -> FsResult<filesystem::types::DescriptorFlags> {
        self.filesystem().get_flags(fd)
    }

    fn get_type(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
    ) -> FsResult<filesystem::types::DescriptorType> {
        self.filesystem().get_type(fd)
    }

    fn set_size(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        size: filesystem::types::Filesize,
    ) -> FsResult<()> {
        self.filesystem().set_size(fd, size)
    }

    fn set_times(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        atim: filesystem::types::NewTimestamp,
        mtim: filesystem::types::NewTimestamp,
    ) -> FsResult<()> {
        self.filesystem().set_times(fd, atim, mtim)
    }

    fn read(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        len: filesystem::types::Filesize,
        offset: filesystem::types::Filesize,
    ) -> FsResult<(Vec<u8>, bool)> {
        self.playback.expect_read_event().map_err(FsError::trap)?;
        self.filesystem().read(fd, len, offset)
    }

    fn write(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        buf: Vec<u8>,
        offset: filesystem::types::Filesize,
    ) -> FsResult<filesystem::types::Filesize> {
        self.filesystem().write(fd, buf, offset)
    }

    fn read_directory(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
    ) -> FsResult<Resource<filesystem::types::DirectoryEntryStream>> {
        self.filesystem().read_directory(fd)
    }

    fn sync(&mut self, fd: Resource<filesystem::types::Descriptor>) -> FsResult<()> {
        self.filesystem().sync(fd)
    }

    fn create_directory_at(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        path: String,
    ) -> FsResult<()> {
        self.filesystem().create_directory_at(fd, path)
    }

    fn stat(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
    ) -> FsResult<filesystem::types::DescriptorStat> {
        self.filesystem().stat(fd)
    }

    fn stat_at(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        path_flags: filesystem::types::PathFlags,
        path: String,
    ) -> FsResult<filesystem::types::DescriptorStat> {
        self.filesystem().stat_at(fd, path_flags, path)
    }

    fn set_times_at(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        path_flags: filesystem::types::PathFlags,
        path: String,
        atim: filesystem::types::NewTimestamp,
        mtim: filesystem::types::NewTimestamp,
    ) -> FsResult<()> {
        self.filesystem()
            .set_times_at(fd, path_flags, path, atim, mtim)
    }

    fn link_at(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        path_flags: filesystem::types::PathFlags,
        old_path: String,
        new_fd: Resource<filesystem::types::Descriptor>,
        new_path: String,
    ) -> FsResult<()> {
        self.filesystem()
            .link_at(fd, path_flags, old_path, new_fd, new_path)
    }

    fn open_at(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        path_flags: filesystem::types::PathFlags,
        path: String,
        open_flags: filesystem::types::OpenFlags,
        descriptor_flags: filesystem::types::DescriptorFlags,
    ) -> FsResult<Resource<filesystem::types::Descriptor>> {
        self.filesystem()
            .open_at(fd, path_flags, path, open_flags, descriptor_flags)
    }

    fn drop(&mut self, fd: Resource<filesystem::types::Descriptor>) -> anyhow::Result<()> {
        let mut fs = self.filesystem();
        filesystem::types::HostDescriptor::drop(&mut fs, fd)
    }

    fn readlink_at(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        path: String,
    ) -> FsResult<String> {
        self.filesystem().readlink_at(fd, path)
    }

    fn remove_directory_at(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        path: String,
    ) -> FsResult<()> {
        self.filesystem().remove_directory_at(fd, path)
    }

    fn rename_at(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        old_path: String,
        new_fd: Resource<filesystem::types::Descriptor>,
        new_path: String,
    ) -> FsResult<()> {
        self.filesystem().rename_at(fd, old_path, new_fd, new_path)
    }

    fn symlink_at(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        old_path: String,
        new_path: String,
    ) -> FsResult<()> {
        self.filesystem().symlink_at(fd, old_path, new_path)
    }

    fn unlink_file_at(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        path: String,
    ) -> FsResult<()> {
        self.filesystem().unlink_file_at(fd, path)
    }

    fn read_via_stream(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        offset: filesystem::types::Filesize,
    ) -> FsResult<Resource<streams::InputStream>> {
        self.filesystem().read_via_stream(fd, offset)
    }

    fn write_via_stream(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        offset: filesystem::types::Filesize,
    ) -> FsResult<Resource<streams::OutputStream>> {
        self.filesystem().write_via_stream(fd, offset)
    }

    fn append_via_stream(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
    ) -> FsResult<Resource<streams::OutputStream>> {
        self.filesystem().append_via_stream(fd)
    }

    fn is_same_object(
        &mut self,
        a: Resource<filesystem::types::Descriptor>,
        b: Resource<filesystem::types::Descriptor>,
    ) -> anyhow::Result<bool> {
        self.filesystem().is_same_object(a, b)
    }

    fn metadata_hash(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
    ) -> FsResult<filesystem::types::MetadataHashValue> {
        self.filesystem().metadata_hash(fd)
    }

    fn metadata_hash_at(
        &mut self,
        fd: Resource<filesystem::types::Descriptor>,
        path_flags: filesystem::types::PathFlags,
        path: String,
    ) -> FsResult<filesystem::types::MetadataHashValue> {
        self.filesystem().metadata_hash_at(fd, path_flags, path)
    }
}

impl filesystem::types::HostDirectoryEntryStream for CtxPlayback {
    fn read_directory_entry(
        &mut self,
        stream: Resource<filesystem::types::DirectoryEntryStream>,
    ) -> FsResult<Option<filesystem::types::DirectoryEntry>> {
        self.filesystem().read_directory_entry(stream)
    }

    fn drop(
        &mut self,
        stream: Resource<filesystem::types::DirectoryEntryStream>,
    ) -> anyhow::Result<()> {
        let mut fs = self.filesystem();
        filesystem::types::HostDirectoryEntryStream::drop(&mut fs, stream)
    }
}
