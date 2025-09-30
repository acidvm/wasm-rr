use std::collections::VecDeque;
use std::fs::File;
use std::path::Path;

use anyhow::anyhow;
use anyhow::Context;
use wasmtime::component::ResourceTable;
use wasmtime_wasi::p2::bindings::{cli, clocks};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

use crate::{Result, TraceEvent, TraceFile};

pub struct Playback {
    events: VecDeque<TraceEvent>,
}

impl Playback {
    pub fn from_file(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("failed to open trace file at {}", path.display()))?;
        let TraceFile { events } = serde_json::from_reader(file)
            .with_context(|| format!("failed to parse trace file at {}", path.display()))?;
        Ok(Self {
            events: events.into(),
        })
    }

    pub fn next_event(&mut self) -> Result<TraceEvent> {
        self.events.pop_front().ok_or(anyhow!("trace exhausted"))
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

    pub fn finish(self) -> Result<()> {
        if self.events.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(
                "trace contains unused events: {:?}",
                self.events.into_iter().collect::<Vec<_>>()
            ))
        }
    }
}

pub struct CtxPlayback {
    table: ResourceTable,
    wasi: WasiCtx,
    playback: Playback,
}

impl CtxPlayback {
    pub fn new(wasi: WasiCtx, playback: Playback) -> Self {
        Self {
            table: ResourceTable::new(),
            wasi,
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

impl clocks::wall_clock::Host for CtxPlayback {
    fn now(&mut self) -> std::result::Result<clocks::wall_clock::Datetime, anyhow::Error> {
        self.playback.next_now()
    }

    fn resolution(&mut self) -> std::result::Result<clocks::wall_clock::Datetime, anyhow::Error> {
        self.playback.next_resolution()
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
