use std::fs::File;
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

use crate::{Result, TraceEvent, TraceFile};

pub struct Recorder {
    output: PathBuf,
    events: Vec<TraceEvent>,
    auto_save: bool,
}

impl Recorder {
    pub fn new(output: PathBuf) -> Self {
        Self {
            output,
            events: Vec::new(),
            auto_save: true,
        }
    }

    pub fn record_now(&mut self, dt: &clocks::wall_clock::Datetime) {
        self.events.push(TraceEvent::ClockNow {
            seconds: dt.seconds,
            nanoseconds: dt.nanoseconds,
        });
    }

    pub fn record_resolution(&mut self, dt: &clocks::wall_clock::Datetime) {
        self.events.push(TraceEvent::ClockResolution {
            seconds: dt.seconds,
            nanoseconds: dt.nanoseconds,
        });
    }

    pub fn record_environment(&mut self, entries: Vec<(String, String)>) {
        self.events.push(TraceEvent::Environment { entries });
    }

    pub fn record_arguments(&mut self, args: Vec<String>) {
        self.events.push(TraceEvent::Arguments { args });
    }

    pub fn record_initial_cwd(&mut self, path: Option<String>) {
        self.events.push(TraceEvent::InitialCwd { path });
    }

    pub fn record_random_bytes(&mut self, bytes: Vec<u8>) {
        self.events.push(TraceEvent::RandomBytes { bytes });
    }

    pub fn record_random_u64(&mut self, value: u64) {
        self.events.push(TraceEvent::RandomU64 { value });
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
        self.events.push(TraceEvent::HttpResponse {
            request_method,
            request_url,
            request_headers,
            status,
            headers,
            body,
        });
    }

    pub fn record_exit(&mut self, code: i32) {
        self.events.push(TraceEvent::Exit { code });
    }

    pub fn save(mut self) -> Result<()> {
        self.auto_save = false; // Disable auto-save since we're manually saving
        let trace = TraceFile {
            events: self.events.clone(), // Clone to avoid move issue
        };

        let file = File::create(&self.output)
            .with_context(|| format!("failed to create trace file at {}", self.output.display()))?;

        serde_json::to_writer_pretty(file, &trace)
            .with_context(|| format!("failed to write trace file at {}", self.output.display()))?;

        Ok(())
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if self.auto_save && !self.events.is_empty() {
            // Try to save the trace on drop, but ignore errors
            let trace = TraceFile {
                events: self.events.clone(),
            };

            if let Ok(file) = File::create(&self.output) {
                let _ = serde_json::to_writer_pretty(file, &trace);
            }
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

        let boxed_body = Full::new(Bytes::from(body_vec))
            .map_err(|_| unreachable!("infallible body error"))
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

impl cli::exit::Host for CtxRecorder {
    fn exit(&mut self, status: std::result::Result<(), ()>) -> anyhow::Result<()> {
        let code = if status.is_ok() { 0 } else { 1 };
        self.recorder.record_exit(code);
        // Still propagate the exit to actually terminate
        self.cli().exit(status)
    }

    fn exit_with_code(&mut self, code: u8) -> anyhow::Result<()> {
        self.recorder.record_exit(code as i32);
        // Still propagate the exit to actually terminate
        self.cli().exit_with_code(code)
    }
}

fn sorted_headers(
    headers: &hyper::HeaderMap,
) -> wasmtime_wasi_http::HttpResult<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    for (name, value) in headers.iter() {
        let value = value
            .to_str()
            .map_err(|err| HttpError::trap(anyhow!("invalid header value for {}: {err}", name)))?;
        pairs.push((name.as_str().to_string(), value.to_string()));
    }
    pairs.sort();
    Ok(pairs)
}
