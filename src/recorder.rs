use std::fs::File;
use std::path::PathBuf;

use anyhow::Context;
use wasmtime::component::ResourceTable;
use wasmtime_wasi::cli::WasiCliView as _;
use wasmtime_wasi::clocks::WasiClocksView as _;
use wasmtime_wasi::p2::bindings::{cli, clocks, random};
use wasmtime_wasi::random::WasiRandomView as _;
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

use crate::{Result, TraceEvent, TraceFile};

pub struct Recorder {
    output: PathBuf,
    events: Vec<TraceEvent>,
}

impl Recorder {
    pub fn new(output: PathBuf) -> Self {
        Self {
            output,
            events: Vec::new(),
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

    // HTTP recording methods - placeholder for future implementation
    // Currently HTTP requests/responses are not intercepted and recorded
    #[allow(dead_code)]
    pub fn record_http_request(&mut self, method: String, url: String, headers: Vec<(String, String)>) {
        self.events.push(TraceEvent::HttpRequest { method, url, headers });
    }

    #[allow(dead_code)]
    pub fn record_http_response(&mut self, status: u16, headers: Vec<(String, String)>, body: Vec<u8>) {
        self.events.push(TraceEvent::HttpResponse { status, headers, body });
    }

    pub fn save(self) -> Result<()> {
        let trace = TraceFile {
            events: self.events,
        };

        let file = File::create(&self.output)
            .with_context(|| format!("failed to create trace file at {}", self.output.display()))?;

        serde_json::to_writer_pretty(file, &trace)
            .with_context(|| format!("failed to write trace file at {}", self.output.display()))?;

        Ok(())
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
