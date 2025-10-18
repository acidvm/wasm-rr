// Enforce strict clippy lints for code quality and safety
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
#![forbid(unsafe_code)]
#![warn(
    clippy::dbg_macro,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::use_debug,
    clippy::exit,
    clippy::indexing_slicing,
    clippy::missing_panics_doc,
    clippy::unwrap_in_result
)]
// TODO: Re-enable after adding comprehensive documentation
#![allow(clippy::missing_errors_doc)]

mod engine;
mod playback;
mod recorder;
mod trace;
mod util;
mod wasi;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use std::path::{Path, PathBuf};
use trace::{convert, TraceFormat};
use wasmtime::component::Component;
use wasmtime::Store;
use wasmtime_wasi::p2::bindings::{cli, clocks, random};
use wasmtime_wasi::p2::bindings::sync::cli as sync_cli;
use wasmtime_wasi::WasiView;
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

#[derive(Parser, Debug)]
#[command(author, version, about, propagate_version = true)]
struct Cli {
    /// Generate markdown help documentation (hidden flag for docs generation)
    #[arg(long, hide = true)]
    markdown_help: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Record all non-deterministic host calls while running the component
    Record {
        /// Path to the component to execute
        wasm: PathBuf,
        /// Output file for the trace (extension determines format: .json or .cbor)
        #[arg(
            short = 't',
            long = "trace",
            value_name = "TRACE",
            default_value = "wasm-rr-trace.json"
        )]
        trace: PathBuf,
        /// Trace format (json or cbor). If not specified, inferred from file extension
        #[arg(
            short = 'f',
            long = "format",
            value_name = "FORMAT",
            value_parser = ["json", "cbor"]
        )]
        format: Option<String>,
        /// Arguments to forward to the component (use `--` to separate)
        #[arg(value_name = "ARGS", num_args = 0.., trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Replay previously recorded host calls from a trace file
    Replay {
        /// Path to the component to execute
        wasm: PathBuf,
        /// Input trace file (extension determines format: .json or .cbor)
        #[arg(default_value = "wasm-rr-trace.json")]
        trace: PathBuf,
        /// Trace format (json or cbor). If not specified, inferred from file extension
        #[arg(
            short = 'f',
            long = "format",
            value_name = "FORMAT",
            value_parser = ["json", "cbor"]
        )]
        format: Option<String>,
    },
    /// Convert a trace file between JSON and CBOR formats
    Convert {
        /// Input trace file
        input: PathBuf,
        /// Output trace file (extension determines format: .json or .cbor)
        output: PathBuf,
        /// Input format (json or cbor). If not specified, inferred from file extension
        #[arg(
            long = "input-format",
            value_name = "FORMAT",
            value_parser = ["json", "cbor"]
        )]
        input_format: Option<String>,
        /// Output format (json or cbor). If not specified, inferred from file extension
        #[arg(
            long = "output-format",
            value_name = "FORMAT",
            value_parser = ["json", "cbor"]
        )]
        output_format: Option<String>,
    },
}

/// Record a WASM component execution, capturing all non-deterministic host calls
fn record(wasm: &Path, trace: &Path, format: TraceFormat, args: &[String]) -> Result<()> {
    let wasi = engine::build_wasi_ctx(wasm, args);
    let http = WasiHttpCtx::new();
    let ctx = recorder::CtxRecorder::new(
        wasi,
        http,
        recorder::Recorder::new(trace.to_path_buf(), format),
    );
    let ctx = run_wasm_with_wasi(wasm, ctx)?;
    ctx.into_recorder().save()
}

/// Replay a previously recorded WASM component execution from a trace file
fn replay(wasm: &Path, trace: &Path, format: TraceFormat) -> Result<()> {
    let playback = playback::Playback::from_file(trace, format)?;
    let wasi = engine::build_wasi_ctx(wasm, &[]);
    let http = WasiHttpCtx::new();
    let ctx = playback::CtxPlayback::new(wasi, http, playback);
    let ctx = run_wasm_with_wasi(wasm, ctx)?;
    ctx.into_playback().finish()
}

fn run_wasm_with_wasi<P, T>(wasm_path: P, ctx: T) -> Result<T>
where
    P: AsRef<Path>,
    T: WasiView
        + WasiHttpView
        + clocks::wall_clock::Host
        + clocks::monotonic_clock::Host
        + cli::environment::Host
        + random::random::Host
        + sync_cli::stdin::Host
        + sync_cli::stdout::Host
        + sync_cli::stderr::Host
        + 'static,
{
    let wasm_path = wasm_path.as_ref();

    let (engine, linker) = engine::configure_engine_and_linker::<T>()?;

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
        .context("Cannot get `wasi:cli/run@0.2.0` interface")?;
    // Get the index for the exported function in the exported interface
    let parent_export_idx = Some(&interface_idx);
    let func_idx = instance
        .get_export_index(&mut store, parent_export_idx, "run")
        .context("Cannot get `run` function in `wasi:cli/run@0.2.0` interface")?;
    let func = instance
        .get_func(&mut store, func_idx)
        .context("Cannot get `run` function handle in `wasi:cli/run@0.2.0`")?;
    // As the `run` function in `wasi:cli/run@0.2.0` takes no argument and return a WASI result that correspond to a `Result<(), ()>`
    // Reference:
    // * https://github.com/WebAssembly/wasi-cli/blob/main/wit/run.wit
    // * Documentation for [Func::typed](https://docs.rs/wasmtime/latest/wasmtime/component/struct.Func.html#method.typed) and [ComponentNamedList](https://docs.rs/wasmtime/latest/wasmtime/component/trait.ComponentNamedList.html)
    let typed = func.typed::<(), (Result<(), ()>,)>(&store)?;

    // Try to call the function, but handle the case where it exits
    match typed.call(&mut store, ()) {
        Ok((result,)) => {
            // Required, see documentation of TypedFunc::call
            typed.post_return(&mut store)?;
            result.map_err(|_| anyhow::anyhow!("error"))?;
        }
        Err(e) => {
            // Check if this is an exit error using proper downcasting
            if e.downcast_ref::<wasmtime_wasi::I32Exit>().is_none() {
                // If it's not an exit error, propagate it
                return Err(e);
            }
            // If it's an exit error, we've already recorded the trace,
            // so we can continue and let the error propagate naturally
        }
    }

    Ok(store.into_data())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle markdown help generation
    if cli.markdown_help {
        use clap_markdown::help_markdown;

        // Allow println for markdown help generation (intentional stdout output)
        #[allow(clippy::print_stdout)]
        {
            println!("# wasm-rr CLI Reference");
            println!();
            println!("This page contains the auto-generated reference documentation for the `wasm-rr` command-line interface.");
            println!();
            println!("{}", help_markdown::<Cli>());
        }
        return Ok(());
    }

    let Some(command) = cli.command else {
        // No subcommand provided, print help and exit with error
        Cli::command().print_help()?;
        std::process::exit(1);
    };

    match command {
        Command::Record {
            wasm,
            trace,
            format,
            args,
        } => {
            let format = TraceFormat::from_path_and_option(&trace, format.as_deref())?;
            record(wasm.as_path(), trace.as_path(), format, &args)
        }
        Command::Replay {
            wasm,
            trace,
            format,
        } => {
            let format = TraceFormat::from_path_and_option(&trace, format.as_deref())?;
            replay(wasm.as_path(), trace.as_path(), format)
        }
        Command::Convert {
            input,
            output,
            input_format,
            output_format,
        } => {
            let input_format = TraceFormat::from_path_and_option(&input, input_format.as_deref())?;
            let output_format =
                TraceFormat::from_path_and_option(&output, output_format.as_deref())?;
            convert(
                input.as_path(),
                output.as_path(),
                input_format,
                output_format,
            )
        }
    }
}
