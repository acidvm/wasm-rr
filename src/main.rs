mod playback;
mod recorder;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use wasmtime::component::{Component, HasData, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::cli::{WasiCli, WasiCliView as _};
use wasmtime_wasi::clocks::{WasiClocks, WasiClocksView as _};
use wasmtime_wasi::filesystem::{WasiFilesystem, WasiFilesystemView as _};
use wasmtime_wasi::p2::bindings::{cli, clocks, random, sync};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};

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
        #[arg(
            short = 't',
            long = "trace",
            value_name = "TRACE",
            default_value = "wasm-rr-trace.json"
        )]
        trace: PathBuf,
        /// Arguments to forward to the component (use `--` to separate)
        #[arg(value_name = "ARGS", num_args = 0.., trailing_var_arg = true)]
        args: Vec<String>,
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
        Command::Record { wasm, trace, args } => record(wasm.as_path(), trace.as_path(), &args),
        Command::Replay { wasm, trace } => replay(wasm.as_path(), trace.as_path()),
    }
}

fn record(wasm: &Path, trace: &Path, args: &[String]) -> Result<()> {
    let wasi = build_wasi_ctx(wasm, args);
    let ctx = recorder::CtxRecorder::new(wasi, recorder::Recorder::new(trace.to_path_buf()));
    let ctx = run_wasm_with_wasi(wasm, ctx)?;
    ctx.into_recorder().save()
}

fn replay(wasm: &Path, trace: &Path) -> Result<()> {
    let playback = playback::Playback::from_file(trace)?;
    let wasi = build_wasi_ctx(wasm, &[]);
    let ctx = playback::CtxPlayback::new(wasi, playback);
    let ctx = run_wasm_with_wasi(wasm, ctx)?;
    ctx.into_playback().finish()
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "call", rename_all = "snake_case")]
enum TraceEvent {
    ClockNow { seconds: u64, nanoseconds: u32 },
    ClockResolution { seconds: u64, nanoseconds: u32 },
    Environment { entries: Vec<(String, String)> },
    Arguments { args: Vec<String> },
    InitialCwd { path: Option<String> },
    RandomBytes { bytes: Vec<u8> },
    RandomU64 { value: u64 },
}

#[derive(Serialize, Deserialize, Debug)]
struct TraceFile {
    events: Vec<TraceEvent>,
}

struct Intercept<Ctx> {
    _marker: PhantomData<Ctx>,
}

impl<Ctx: 'static> HasData for Intercept<Ctx> {
    type Data<'a> = &'a mut Ctx;
}

struct HasIo;

impl HasData for HasIo {
    type Data<'a> = &'a mut ResourceTable;
}

fn build_wasi_ctx(wasm_path: &Path, args: &[String]) -> WasiCtx {
    let mut builder = WasiCtxBuilder::new();
    builder.inherit_stdio();

    let program_name = wasm_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("component");
    builder.arg(program_name);
    for arg in args {
        builder.arg(arg);
    }

    builder.build()
}

fn run_wasm_with_wasi<P, T>(wasm_path: P, ctx: T) -> Result<T>
where
    P: AsRef<Path>,
    T: WasiView + clocks::wall_clock::Host + cli::environment::Host + random::random::Host + 'static,
{
    let wasm_path = wasm_path.as_ref();

    let (engine, linker) = configure_engine_and_linker::<T>()?;

    let mut store = Store::new(&engine, ctx);

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

    Ok(store.into_data())
}

fn configure_engine_and_linker<T>() -> Result<(Engine, Linker<T>)>
where
    T: WasiView + clocks::wall_clock::Host + cli::environment::Host + random::random::Host + 'static,
{
    // Create an engine with the component model enabled and a component linker.
    let mut config = Config::new();
    config.wasm_component_model(true);
    let engine = Engine::new(&config).context("failed to create engine with component model")?;
    let mut linker: Linker<T> = Linker::new(&engine);

    // Wire required WASI Preview2 imports explicitly, with custom clocks.
    // I/O
    sync::io::streams::add_to_linker::<T, HasIo>(&mut linker, |t| t.ctx().table)
        .context("failed to add wasi:io/streams")?;
    sync::io::error::add_to_linker::<T, HasIo>(&mut linker, |t| t.ctx().table)
        .context("failed to add wasi:io/error")?;
    // CLI
    cli::environment::add_to_linker::<T, Intercept<T>>(&mut linker, |t| t)
        .context("failed to add wasi:cli/environment")?;
    cli::stdin::add_to_linker::<T, WasiCli>(&mut linker, |t| t.cli())
        .context("failed to add wasi:cli/stdin")?;
    cli::stdout::add_to_linker::<T, WasiCli>(&mut linker, |t| t.cli())
        .context("failed to add wasi:cli/stdout")?;
    cli::stderr::add_to_linker::<T, WasiCli>(&mut linker, |t| t.cli())
        .context("failed to add wasi:cli/stderr")?;
    cli::exit::add_to_linker::<T, WasiCli>(
        &mut linker,
        &wasmtime_wasi::p2::bindings::sync::LinkOptions::default().into(),
        |t| t.cli(),
    )
    .context("failed to add wasi:cli/exit")?;

    // Filesystem (types + preopens)
    sync::filesystem::types::add_to_linker::<T, WasiFilesystem>(&mut linker, |t| t.filesystem())
        .context("failed to add wasi:filesystem/types")?;
    sync::filesystem::preopens::add_to_linker::<T, WasiFilesystem>(&mut linker, |t| t.filesystem())
        .context("failed to add wasi:filesystem/preopens")?;
    // Clocks (custom host implementation)
    clocks::wall_clock::add_to_linker::<T, Intercept<T>>(&mut linker, |s| s)
        .context("failed to add wasi:clocks/wall-clock")?;
    clocks::monotonic_clock::add_to_linker::<T, WasiClocks>(&mut linker, |t| t.clocks())
        .context("failed to add wasi:clocks/monotonic-clock")?;

    // Random (custom host implementation)
    random::random::add_to_linker::<T, Intercept<T>>(&mut linker, |s| s)
        .context("failed to add wasi:random/random")?;

    Ok((engine, linker))
}
