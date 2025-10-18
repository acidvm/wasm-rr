use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use anyhow::{anyhow, Context};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use wasmtime::component::ResourceTable;
use wasmtime_wasi::cli::WasiCliView;
use wasmtime_wasi::clocks::WasiClocksView as _;
use wasmtime_wasi::p2::bindings::{cli, clocks, random};
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

    // TODO: These will be called when filesystem interception is implemented
    #[allow(dead_code)]
    pub fn record_descriptor_read(&mut self) {
        self.write_event(TraceEvent::DescriptorRead);
    }

    #[allow(dead_code)]
    pub fn record_descriptor_write(&mut self) {
        self.write_event(TraceEvent::DescriptorWrite);
    }

    #[allow(dead_code)]
    pub fn record_descriptor_seek(&mut self) {
        self.write_event(TraceEvent::DescriptorSeek);
    }

    #[allow(dead_code)]
    pub fn record_descriptor_open_at(&mut self) {
        self.write_event(TraceEvent::DescriptorOpenAt);
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
