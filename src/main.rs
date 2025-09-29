use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::File;
use std::mem;
use std::path::{Path, PathBuf};
use wasmtime::component::{Component, HasData, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::cli::{WasiCli, WasiCliView as _};
use wasmtime_wasi::clocks::{WasiClocks, WasiClocksView as _};
use wasmtime_wasi::filesystem::{WasiFilesystem, WasiFilesystemView as _};
use wasmtime_wasi::p2::bindings::{cli, clocks, sync};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

#[derive(Parser, Debug)]
#[command(author, version, about, propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Record all `wasi:clocks/wall-clock.now` calls while running the component
    Record {
        /// Path to the component to execute
        wasm: PathBuf,
        /// Output file for the trace JSON
        #[arg(default_value = "wasm-rr-trace.json")]
        trace: PathBuf,
    },
    /// Replay previously recorded clock values from a trace file
    Replay {
        /// Path to the component to execute
        wasm: PathBuf,
        /// Input trace JSON file
        #[arg(default_value = "wasm-rr-trace.json")]
        trace: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Record { wasm, trace } => record(wasm.as_path(), trace.as_path()),
        Command::Replay { wasm, trace } => replay(wasm.as_path(), trace.as_path()),
    }
}

fn record(wasm: &Path, trace: &Path) -> Result<()> {
    let mode = Mode::Record(Recorder::new(trace.to_path_buf()));
    match run_wasm_with_wasi(wasm, mode)? {
        Mode::Record(recorder) => recorder.save(),
        _ => unreachable!("mode changed during record"),
    }
}

fn replay(wasm: &Path, trace: &Path) -> Result<()> {
    let playback = Playback::from_file(trace)?;
    match run_wasm_with_wasi(wasm, Mode::Replay(playback))? {
        Mode::Replay(playback) => playback.finish(),
        _ => unreachable!("mode changed during replay"),
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "call", rename_all = "snake_case")]
enum ClockCall {
    Now { seconds: u64, nanoseconds: u32 },
    Resolution { seconds: u64, nanoseconds: u32 },
}

#[derive(Serialize, Deserialize, Debug)]
struct TraceFile {
    calls: Vec<ClockCall>,
}

struct Recorder {
    output: PathBuf,
    calls: Vec<ClockCall>,
}

impl Recorder {
    fn new(output: PathBuf) -> Self {
        Self {
            output,
            calls: Vec::new(),
        }
    }

    fn record_now(&mut self, dt: &clocks::wall_clock::Datetime) {
        self.calls.push(ClockCall::Now {
            seconds: dt.seconds,
            nanoseconds: dt.nanoseconds,
        });
    }

    fn record_resolution(&mut self, dt: &clocks::wall_clock::Datetime) {
        self.calls.push(ClockCall::Resolution {
            seconds: dt.seconds,
            nanoseconds: dt.nanoseconds,
        });
    }

    fn save(self) -> Result<()> {
        let trace = TraceFile { calls: self.calls };

        let file = File::create(&self.output)
            .with_context(|| format!("failed to create trace file at {}", self.output.display()))?;

        serde_json::to_writer_pretty(file, &trace)
            .with_context(|| format!("failed to write trace file at {}", self.output.display()))?;

        Ok(())
    }
}

struct Playback {
    calls: VecDeque<ClockCall>,
}

impl Playback {
    fn from_file(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("failed to open trace file at {}", path.display()))?;
        let TraceFile { calls } = serde_json::from_reader(file)
            .with_context(|| format!("failed to parse trace file at {}", path.display()))?;
        Ok(Self {
            calls: calls.into(),
        })
    }

    fn next_now(&mut self) -> Result<clocks::wall_clock::Datetime> {
        match self.calls.pop_front() {
            Some(ClockCall::Now {
                seconds,
                nanoseconds,
            }) => Ok(clocks::wall_clock::Datetime {
                seconds,
                nanoseconds,
            }),
            Some(other) => Err(anyhow!(
                "expected next clock event to be 'now', got {:?}",
                other
            )),
            None => Err(anyhow!("trace exhausted before next 'now' value")),
        }
    }

    fn next_resolution(&mut self) -> Result<clocks::wall_clock::Datetime> {
        match self.calls.pop_front() {
            Some(ClockCall::Resolution {
                seconds,
                nanoseconds,
            }) => Ok(clocks::wall_clock::Datetime {
                seconds,
                nanoseconds,
            }),
            Some(other) => Err(anyhow!(
                "expected next clock event to be 'resolution', got {:?}",
                other
            )),
            None => Err(anyhow!("trace exhausted before next 'resolution' value")),
        }
    }

    fn finish(self) -> Result<()> {
        if self.calls.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(
                "trace contains unused events: {:?}",
                self.calls.into_iter().collect::<Vec<_>>()
            ))
        }
    }
}

enum Mode {
    Record(Recorder),
    Replay(Playback),
    Passthrough,
}

struct Ctx {
    table: ResourceTable,
    wasi: WasiCtx,
    mode: Mode,
}

impl WasiView for Ctx {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl clocks::wall_clock::Host for Ctx {
    fn now(&mut self) -> std::result::Result<clocks::wall_clock::Datetime, anyhow::Error> {
        let mode = mem::replace(&mut self.mode, Mode::Passthrough);

        let result = match mode {
            Mode::Record(mut recorder) => {
                let now = self.clocks().now()?;
                recorder.record_now(&now);
                self.mode = Mode::Record(recorder);
                Ok(now)
            }
            Mode::Replay(mut playback) => {
                let value = playback.next_now();
                self.mode = Mode::Replay(playback);
                value
            }
            Mode::Passthrough => {
                let now = self.clocks().now();
                self.mode = Mode::Passthrough;
                now
            }
        };

        result
    }

    fn resolution(&mut self) -> std::result::Result<clocks::wall_clock::Datetime, anyhow::Error> {
        let mode = mem::replace(&mut self.mode, Mode::Passthrough);

        let result = match mode {
            Mode::Record(mut recorder) => {
                let resolution = self.clocks().resolution()?;
                recorder.record_resolution(&resolution);
                self.mode = Mode::Record(recorder);
                Ok(resolution)
            }
            Mode::Replay(mut playback) => {
                let value = playback.next_resolution();
                self.mode = Mode::Replay(playback);
                value
            }
            Mode::Passthrough => {
                let resolution = self.clocks().resolution();
                self.mode = Mode::Passthrough;
                resolution
            }
        };

        result
    }
}

struct WallClock;

impl HasData for WallClock {
    type Data<'a> = &'a mut Ctx;
}

struct HasIo;

impl HasData for HasIo {
    type Data<'a> = &'a mut ResourceTable;
}

fn run_wasm_with_wasi<P: AsRef<Path>>(wasm_path: P, mode: Mode) -> Result<Mode> {
    let wasm_path = wasm_path.as_ref();

    // Create an engine with the component model enabled and a component linker.
    let mut config = Config::new();
    config.wasm_component_model(true);
    let engine = Engine::new(&config).context("failed to create engine with component model")?;
    let mut linker: Linker<Ctx> = Linker::new(&engine);

    // Build a minimal WASI context that inherits stdio from the host.
    let mut wasi_ctx_builder = WasiCtxBuilder::new();
    wasi_ctx_builder.inherit_stdio();
    let wasi_ctx = wasi_ctx_builder.build();

    // Wire required WASI Preview2 imports explicitly, with custom clocks.
    // I/O
    sync::io::streams::add_to_linker::<Ctx, HasIo>(&mut linker, |t| t.ctx().table)
        .context("failed to add wasi:io/streams")?;
    sync::io::error::add_to_linker::<Ctx, HasIo>(&mut linker, |t| t.ctx().table)
        .context("failed to add wasi:io/error")?;
    // CLI
    cli::environment::add_to_linker::<Ctx, WasiCli>(&mut linker, Ctx::cli)
        .context("failed to add wasi:cli/environment")?;
    cli::stdin::add_to_linker::<Ctx, WasiCli>(&mut linker, Ctx::cli)
        .context("failed to add wasi:cli/stdin")?;
    cli::stdout::add_to_linker::<Ctx, WasiCli>(&mut linker, Ctx::cli)
        .context("failed to add wasi:cli/stdout")?;
    cli::stderr::add_to_linker::<Ctx, WasiCli>(&mut linker, Ctx::cli)
        .context("failed to add wasi:cli/stderr")?;
    cli::exit::add_to_linker::<Ctx, WasiCli>(
        &mut linker,
        &wasmtime_wasi::p2::bindings::sync::LinkOptions::default().into(),
        Ctx::cli,
    )
    .context("failed to add wasi:cli/exit")?;

    // Filesystem (types + preopens)
    sync::filesystem::types::add_to_linker::<Ctx, WasiFilesystem>(&mut linker, Ctx::filesystem)
        .context("failed to add wasi:filesystem/types")?;
    sync::filesystem::preopens::add_to_linker::<Ctx, WasiFilesystem>(&mut linker, Ctx::filesystem)
        .context("failed to add wasi:filesystem/preopens")?;
    // Clocks (custom host implementation)
    clocks::wall_clock::add_to_linker::<Ctx, WallClock>(&mut linker, |s: &mut Ctx| s)
        .context("failed to add wasi:clocks/wall-clock")?;
    clocks::monotonic_clock::add_to_linker::<Ctx, WasiClocks>(&mut linker, Ctx::clocks)
        .context("failed to add wasi:clocks/monotonic-clock")?;

    let mut store = Store::new(
        &engine,
        Ctx {
            table: ResourceTable::new(),
            wasi: wasi_ctx,
            mode,
        },
    );

    // Compile and instantiate the component.
    let component = Component::from_file(&engine, wasm_path)
        .with_context(|| format!("failed to read/compile component: {}", wasm_path.display()))?;

    let instance = linker
        .instantiate(&mut store, &component)
        .context("failed to instantiate component")?;

    // Get the index for the exported interface
    let interface_idx = instance
        .get_export_index(&mut store, None, "wasi:cli/run@0.2.0")
        .expect("Cannot get `wasi:cli/run@0.2.0` interface");
    // Get the index for the exported function in the exported interface
    let parent_export_idx = Some(&interface_idx);
    let func_idx = instance
        .get_export_index(&mut store, parent_export_idx, "run")
        .expect("Cannot get `run` function in `wasi:cli/run@0.2.0` interface");
    let func = instance
        .get_func(&mut store, func_idx)
        .expect("Unreachable since we've got func_idx");
    // As the `run` function in `wasi:cli/run@0.2.0` takes no argument and return a WASI result that correspond to a `Result<(), ()>`
    // Reference:
    // * https://github.com/WebAssembly/wasi-cli/blob/main/wit/run.wit
    // * Documentation for [Func::typed](https://docs.rs/wasmtime/latest/wasmtime/component/struct.Func.html#method.typed) and [ComponentNamedList](https://docs.rs/wasmtime/latest/wasmtime/component/trait.ComponentNamedList.html)
    let typed = func.typed::<(), (Result<(), ()>,)>(&store)?;
    let (result,) = typed.call(&mut store, ())?;
    // Required, see documentation of TypedFunc::call
    typed.post_return(&mut store)?;
    result.map_err(|_| anyhow::anyhow!("error"))?;

    let Ctx {
        mode: store_mode, ..
    } = store.into_data();
    Ok(store_mode)
}
