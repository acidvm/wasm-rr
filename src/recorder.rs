use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use anyhow::{anyhow, Context};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use wasmtime::component::{Resource, ResourceTable};
use wasmtime_wasi::cli::WasiCliView;
use wasmtime_wasi::clocks::WasiClocksView as _;
use wasmtime_wasi::filesystem::WasiFilesystemView as _;
use wasmtime_wasi::p2::bindings::sync::io::{poll, streams};
use wasmtime_wasi::p2::bindings::{cli, clocks, random, sync::filesystem};
use wasmtime_wasi::p2::{FsResult, StreamError, StreamResult};
use wasmtime_wasi::random::WasiRandomView as _;
use wasmtime_wasi::runtime;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::types::{
    default_send_request, HostFutureIncomingResponse, IncomingResponse, OutgoingRequestConfig,
};
use wasmtime_wasi_http::{HttpError, WasiHttpCtx, WasiHttpView};

use crate::trace::{TraceEvent, TraceFormat};
use crate::wasi::util::sorted_headers;
use anyhow::Result;

enum TraceWriter {
    Json {
        writer: BufWriter<File>,
        first: bool,
    },
    Cbor {
        writer: BufWriter<File>,
    },
}

impl TraceWriter {
    fn new(output: PathBuf, format: TraceFormat) -> Result<Self> {
        let file = File::create(&output)
            .with_context(|| format!("failed to create trace file at {}", output.display()))?;
        let writer = BufWriter::new(file);

        match format {
            TraceFormat::Json => {
                let mut writer = writer;
                // Write the beginning of the JSON array
                writer
                    .write_all(b"{\"events\":[\n")
                    .context("failed to write JSON header")?;
                Ok(TraceWriter::Json {
                    writer,
                    first: true,
                })
            }
            TraceFormat::Cbor => Ok(TraceWriter::Cbor { writer }),
        }
    }

    fn write_event(&mut self, event: &TraceEvent) -> Result<()> {
        match self {
            TraceWriter::Json { writer, first } => {
                if !*first {
                    writer.write_all(b",\n")?;
                }
                *first = false;
                serde_json::to_writer(&mut *writer, event)?;
                writer.flush()?;
                Ok(())
            }
            TraceWriter::Cbor { writer } => {
                ciborium::into_writer(event, &mut *writer)?;
                writer.flush()?;
                Ok(())
            }
        }
    }

    fn finish(self) -> Result<()> {
        match self {
            TraceWriter::Json { mut writer, .. } => {
                writer.write_all(b"\n]}")?;
                writer.flush()?;
                Ok(())
            }
            TraceWriter::Cbor { mut writer } => {
                writer.flush()?;
                Ok(())
            }
        }
    }
}

pub struct Recorder {
    writer: Option<TraceWriter>,
    error: Option<anyhow::Error>,
}

impl Recorder {
    pub fn new(output: PathBuf, format: TraceFormat) -> Self {
        match TraceWriter::new(output, format) {
            Ok(writer) => Self {
                writer: Some(writer),
                error: None,
            },
            Err(e) => Self {
                writer: None,
                error: Some(e),
            },
        }
    }

    fn write_event(&mut self, event: TraceEvent) {
        if self.error.is_some() {
            return;
        }
        if let Some(writer) = &mut self.writer {
            if let Err(e) = writer.write_event(&event) {
                self.error = Some(e);
            }
        }
    }

    pub fn record_now(&mut self, dt: &clocks::wall_clock::Datetime) {
        self.write_event(TraceEvent::ClockNow {
            seconds: dt.seconds,
            nanoseconds: dt.nanoseconds,
        });
    }

    pub fn record_resolution(&mut self, dt: &clocks::wall_clock::Datetime) {
        self.write_event(TraceEvent::ClockResolution {
            seconds: dt.seconds,
            nanoseconds: dt.nanoseconds,
        });
    }

    pub fn record_monotonic_now(&mut self, nanoseconds: u64) {
        self.write_event(TraceEvent::MonotonicClockNow { nanoseconds });
    }

    pub fn record_monotonic_resolution(&mut self, nanoseconds: u64) {
        self.write_event(TraceEvent::MonotonicClockResolution { nanoseconds });
    }

    pub fn record_environment(&mut self, entries: Vec<(String, String)>) {
        self.write_event(TraceEvent::Environment { entries });
    }

    pub fn record_arguments(&mut self, args: Vec<String>) {
        self.write_event(TraceEvent::Arguments { args });
    }

    pub fn record_initial_cwd(&mut self, path: Option<String>) {
        self.write_event(TraceEvent::InitialCwd { path });
    }

    pub fn record_random_bytes(&mut self, bytes: Vec<u8>) {
        self.write_event(TraceEvent::RandomBytes { bytes });
    }

    pub fn record_random_u64(&mut self, value: u64) {
        self.write_event(TraceEvent::RandomU64 { value });
    }

    pub fn record_filesystem_read(&mut self) {
        self.write_event(TraceEvent::Read);
    }

    pub fn record_http_response(
        &mut self,
        request_method: String,
        request_url: String,
        request_headers: Vec<(String, String)>,
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) {
        self.write_event(TraceEvent::HttpResponse {
            request_method,
            request_url,
            request_headers,
            status,
            headers,
            body,
        });
    }

    pub fn save(mut self) -> Result<()> {
        if let Some(error) = self.error.take() {
            return Err(error);
        }
        if let Some(writer) = self.writer.take() {
            writer.finish()
        } else {
            Err(anyhow!("recorder has no writer"))
        }
    }
}

pub struct CtxRecorder {
    table: ResourceTable,
    wasi: WasiCtx,
    http: WasiHttpCtx,
    recorder: Recorder,
}

impl CtxRecorder {
    pub fn new(wasi: WasiCtx, http: WasiHttpCtx, recorder: Recorder) -> Self {
        Self {
            table: ResourceTable::new(),
            wasi,
            http,
            recorder,
        }
    }

    pub fn into_recorder(self) -> Recorder {
        self.recorder
    }
}

impl WasiView for CtxRecorder {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for CtxRecorder {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn send_request(
        &mut self,
        request: hyper::Request<wasmtime_wasi_http::body::HyperOutgoingBody>,
        config: OutgoingRequestConfig,
    ) -> wasmtime_wasi_http::HttpResult<HostFutureIncomingResponse> {
        let method = request.method().to_string();
        let url = request.uri().to_string();
        let request_headers = sorted_headers(request.headers())?;

        let future = default_send_request(request, config);

        let result = match future {
            HostFutureIncomingResponse::Pending(handle) => runtime::in_tokio(handle),
            HostFutureIncomingResponse::Ready(res) => res,
            HostFutureIncomingResponse::Consumed => {
                return Err(HttpError::trap(anyhow!(
                    "unexpected consumed HTTP response handle"
                )))
            }
        };

        let result = result.map_err(HttpError::trap)?;

        let incoming = match result {
            Ok(resp) => resp,
            Err(code) => {
                return Ok(HostFutureIncomingResponse::ready(Ok(Err(code))));
            }
        };

        let between_bytes_timeout = incoming.between_bytes_timeout;
        let (parts, body) = incoming.resp.into_parts();

        let recorded_headers = sorted_headers(&parts.headers)?;

        let bytes = runtime::in_tokio(async move { body.collect().await })
            .map_err(HttpError::trap)?
            .to_bytes();

        let body_vec = bytes.to_vec();

        self.recorder.record_http_response(
            method,
            url,
            request_headers,
            parts.status.as_u16(),
            recorded_headers,
            body_vec.clone(),
        );

        // Full<Bytes> is infallible, but we need to convert the error type to match the expected signature.
        // We use an explicit match on Infallible rather than Into::into because ErrorCode doesn't
        // implement From<Infallible>.
        let boxed_body = Full::new(Bytes::from(body_vec))
            .map_err(|e: std::convert::Infallible| match e {})
            .boxed();

        let mut builder = hyper::Response::builder().status(parts.status);
        *builder
            .headers_mut()
            .ok_or_else(|| HttpError::trap(anyhow!("failed to access response headers")))? =
            parts.headers.clone();

        let resp = builder.body(boxed_body).map_err(HttpError::trap)?;

        let incoming_response = IncomingResponse {
            resp,
            worker: None,
            between_bytes_timeout,
        };

        Ok(HostFutureIncomingResponse::ready(Ok(Ok(incoming_response))))
    }
}

impl clocks::wall_clock::Host for CtxRecorder {
    fn now(&mut self) -> std::result::Result<clocks::wall_clock::Datetime, anyhow::Error> {
        let now = self.clocks().now()?;
        self.recorder.record_now(&now);
        Ok(now)
    }

    fn resolution(&mut self) -> std::result::Result<clocks::wall_clock::Datetime, anyhow::Error> {
        let resolution = self.clocks().resolution()?;
        self.recorder.record_resolution(&resolution);
        Ok(resolution)
    }
}

impl clocks::monotonic_clock::Host for CtxRecorder {
    fn now(&mut self) -> anyhow::Result<u64> {
        let now = self.clocks().now()?;
        self.recorder.record_monotonic_now(now);
        Ok(now)
    }

    fn resolution(&mut self) -> anyhow::Result<u64> {
        let resolution = self.clocks().resolution()?;
        self.recorder.record_monotonic_resolution(resolution);
        Ok(resolution)
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
        self.clocks().subscribe_duration(duration)
    }
}

impl cli::environment::Host for CtxRecorder {
    fn get_environment(&mut self) -> anyhow::Result<Vec<(String, String)>> {
        let env = self.cli().get_environment()?;
        self.recorder.record_environment(env.clone());
        Ok(env)
    }

    fn get_arguments(&mut self) -> anyhow::Result<Vec<String>> {
        let args = self.cli().get_arguments()?;
        self.recorder.record_arguments(args.clone());
        Ok(args)
    }

    fn initial_cwd(&mut self) -> anyhow::Result<Option<String>> {
        let cwd = self.cli().initial_cwd()?;
        self.recorder.record_initial_cwd(cwd.clone());
        Ok(cwd)
    }
}

impl random::random::Host for CtxRecorder {
    fn get_random_bytes(&mut self, len: u64) -> anyhow::Result<Vec<u8>> {
        let bytes = self.random().get_random_bytes(len)?;
        self.recorder.record_random_bytes(bytes.clone());
        Ok(bytes)
    }

    fn get_random_u64(&mut self) -> anyhow::Result<u64> {
        let value = self.random().get_random_u64()?;
        self.recorder.record_random_u64(value);
        Ok(value)
    }
}

impl streams::Host for CtxRecorder {
    fn convert_stream_error(&mut self, err: StreamError) -> anyhow::Result<streams::StreamError> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::Host>::convert_stream_error(view.table, err)
    }
}

impl streams::HostInputStream for CtxRecorder {
    fn drop(&mut self, stream: Resource<streams::InputStream>) -> anyhow::Result<()> {
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostInputStream>::drop(view.table, stream)
    }

    fn read(&mut self, stream: Resource<streams::InputStream>, len: u64) -> StreamResult<Vec<u8>> {
        self.recorder.record_filesystem_read();
        let view = WasiView::ctx(self);
        <ResourceTable as streams::HostInputStream>::read(view.table, stream, len)
    }

    fn blocking_read(
        &mut self,
        stream: Resource<streams::InputStream>,
        len: u64,
    ) -> StreamResult<Vec<u8>> {
        self.recorder.record_filesystem_read();
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

impl streams::HostOutputStream for CtxRecorder {
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

impl filesystem::types::Host for CtxRecorder {
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

impl filesystem::types::HostDescriptor for CtxRecorder {
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
        self.recorder.record_filesystem_read();
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

impl filesystem::types::HostDirectoryEntryStream for CtxRecorder {
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
