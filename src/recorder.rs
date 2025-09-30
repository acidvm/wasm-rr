use std::fs::File;
use std::path::PathBuf;

use anyhow::Context;
use wasmtime::component::ResourceTable;
use wasmtime_wasi::cli::WasiCliView as _;
use wasmtime_wasi::clocks::WasiClocksView as _;
use wasmtime_wasi::p2::bindings::{cli, clocks};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

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
    recorder: Recorder,
}

impl CtxRecorder {
    pub fn new(wasi: WasiCtx, recorder: Recorder) -> Self {
        Self {
            table: ResourceTable::new(),
            wasi,
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
