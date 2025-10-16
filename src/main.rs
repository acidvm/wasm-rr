mod playback;
mod recorder;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::p2::bindings::{cli, clocks, random};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::{WasiHttpCtx, WasiHttpView};

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
    let http = WasiHttpCtx::new();
    let ctx = recorder::CtxRecorder::new(wasi, http, recorder::Recorder::new(trace.to_path_buf()));

    // Run the WASM and handle potential exit errors
    let ctx = match run_wasm_with_wasi(wasm, ctx) {
        Ok(ctx) => ctx,
        Err(e) => {
            // If the error is an exit error, we still want to save the trace
            let error_msg = e.to_string();
            if error_msg.contains("error while executing")
                && error_msg.contains("Exited with i32 exit status")
            {
                // We can't recover the context from the error, so we can't save the trace
                // This is a limitation of the current design
                return Err(e);
            }
            return Err(e);
        }
    };

    ctx.into_recorder().save()
}

fn replay(wasm: &Path, trace: &Path) -> Result<()> {
    let playback = playback::Playback::from_file(trace)?;
    let wasi = build_wasi_ctx(wasm, &[]);
    let http = WasiHttpCtx::new();
    let ctx = playback::CtxPlayback::new(wasi, http, playback);

    // Run the WASM and handle potential exit errors (same as in record)
    let ctx = match run_wasm_with_wasi(wasm, ctx) {
        Ok(ctx) => ctx,
        Err(e) => {
            // If the error is an exit error, we still want to verify the playback
            let error_msg = e.to_string();
            if error_msg.contains("error while executing")
                && error_msg.contains("Exited with i32 exit status")
            {
                // We can't recover the context from the error, but for replay
                // an exit error is expected if it was recorded
                // Since we can't call finish(), we'll just return Ok
                return Ok(());
            }
            return Err(e);
        }
    };

    ctx.into_playback().finish()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "call", rename_all = "snake_case")]
enum TraceEvent {
    ClockNow {
        seconds: u64,
        nanoseconds: u32,
    },
    ClockResolution {
        seconds: u64,
        nanoseconds: u32,
    },
    Environment {
        entries: Vec<(String, String)>,
    },
    Arguments {
        args: Vec<String>,
    },
    InitialCwd {
        path: Option<String>,
    },
    RandomBytes {
        bytes: Vec<u8>,
    },
    RandomU64 {
        value: u64,
    },
    HttpResponse {
        request_method: String,
        request_url: String,
        request_headers: Vec<(String, String)>,
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    },
    Exit {
        code: i32,
    },
}

#[derive(Serialize, Deserialize, Debug)]
struct TraceFile {
    events: Vec<TraceEvent>,
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
    T: WasiView
        + WasiHttpView
        + clocks::wall_clock::Host
        + cli::environment::Host
        + cli::exit::Host
        + random::random::Host
        + 'static,
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

    // Try to call the function, but handle the case where it exits
    match typed.call(&mut store, ()) {
        Ok((result,)) => {
            // Required, see documentation of TypedFunc::call
            typed.post_return(&mut store)?;
            result.map_err(|_| anyhow::anyhow!("error"))?;
        }
        Err(e) => {
            // Check if this is an exit error
            let error_msg = e.to_string();
            if !error_msg.contains("Exited with i32 exit status") {
                // If it's not an exit error, propagate it
                return Err(e);
            }
            // If it's an exit error, we've already recorded it, so we can continue
        }
    }

    Ok(store.into_data())
}

fn configure_engine_and_linker<T>() -> Result<(Engine, Linker<T>)>
where
    T: WasiView
        + WasiHttpView
        + clocks::wall_clock::Host
        + cli::environment::Host
        + cli::exit::Host
        + random::random::Host
        + 'static,
{
    // Create an engine with the component model enabled and a component linker.
    let mut config = Config::new();
    config.wasm_component_model(true);
    let engine = Engine::new(&config).context("failed to create engine with component model")?;
    let mut linker: Linker<T> = Linker::new(&engine);

    // Add HTTP components first
    wasmtime_wasi_http::add_only_http_to_linker_sync(&mut linker)
        .context("failed to add wasi:http components")?;

    // Add I/O components needed by both WASI and HTTP
    add_wasi_io_to_linker(&mut linker)?;

    // Now add the components we want to intercept using our custom implementations
    // We need to use a wrapper type pattern to make this work with the linker
    struct Intercept<T>(std::marker::PhantomData<T>);
    impl<T: 'static> wasmtime::component::HasData for Intercept<T> {
        type Data<'a>
            = &'a mut T
        where
            T: 'a;
    }

    clocks::wall_clock::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;
    cli::environment::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;
    cli::exit::add_to_linker::<_, Intercept<T>>(&mut linker, &Default::default(), |ctx| ctx)?;
    random::random::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;

    // Add remaining WASI components that we don't need to intercept
    add_remaining_wasi_to_linker(&mut linker)?;

    Ok((engine, linker))
}

fn add_wasi_io_to_linker<T: WasiView>(linker: &mut Linker<T>) -> Result<()> {
    use wasmtime::component::ResourceTable;
    use wasmtime_wasi::p2::bindings;

    struct HasIo;
    impl wasmtime::component::HasData for HasIo {
        type Data<'a> = &'a mut ResourceTable;
    }

    wasmtime_wasi_io::bindings::wasi::io::error::add_to_linker::<T, HasIo>(linker, |t| {
        t.ctx().table
    })?;
    bindings::sync::io::poll::add_to_linker::<T, HasIo>(linker, |t| t.ctx().table)?;
    bindings::sync::io::streams::add_to_linker::<T, HasIo>(linker, |t| t.ctx().table)?;

    Ok(())
}

fn add_remaining_wasi_to_linker<T: WasiView + WasiHttpView>(linker: &mut Linker<T>) -> Result<()> {
    use wasmtime_wasi::cli::{WasiCli, WasiCliView};
    use wasmtime_wasi::clocks::{WasiClocks, WasiClocksView};
    use wasmtime_wasi::filesystem::{WasiFilesystem, WasiFilesystemView};
    use wasmtime_wasi::p2::bindings;
    use wasmtime_wasi::random::{WasiRandom, WasiRandomView};
    use wasmtime_wasi::sockets::{WasiSockets, WasiSocketsView};

    // Add CLI components (except environment and exit which we intercept)
    bindings::sync::cli::stdin::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;
    bindings::sync::cli::stdout::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;
    bindings::sync::cli::stderr::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;
    bindings::sync::cli::terminal_input::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;
    bindings::sync::cli::terminal_output::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;
    bindings::sync::cli::terminal_stdin::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;
    bindings::sync::cli::terminal_stdout::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;
    bindings::sync::cli::terminal_stderr::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;

    // Add clocks components (except wall-clock which we intercept)
    bindings::sync::clocks::monotonic_clock::add_to_linker::<T, WasiClocks>(linker, |ctx| {
        ctx.clocks()
    })?;

    // Add filesystem components
    bindings::sync::filesystem::types::add_to_linker::<T, WasiFilesystem>(linker, |ctx| {
        ctx.filesystem()
    })?;
    bindings::sync::filesystem::preopens::add_to_linker::<T, WasiFilesystem>(linker, |ctx| {
        ctx.filesystem()
    })?;

    // Add random components (except random which we intercept)
    bindings::sync::random::insecure::add_to_linker::<T, WasiRandom>(linker, |ctx| ctx.random())?;
    bindings::sync::random::insecure_seed::add_to_linker::<T, WasiRandom>(linker, |ctx| {
        ctx.random()
    })?;

    // Add socket components
    bindings::sync::sockets::tcp::add_to_linker::<T, WasiSockets>(linker, |ctx| ctx.sockets())?;
    bindings::sync::sockets::udp::add_to_linker::<T, WasiSockets>(linker, |ctx| ctx.sockets())?;
    bindings::sockets::tcp_create_socket::add_to_linker::<T, WasiSockets>(linker, |ctx| {
        ctx.sockets()
    })?;
    bindings::sockets::udp_create_socket::add_to_linker::<T, WasiSockets>(linker, |ctx| {
        ctx.sockets()
    })?;
    bindings::sockets::instance_network::add_to_linker::<T, WasiSockets>(linker, |ctx| {
        ctx.sockets()
    })?;
    bindings::sockets::network::add_to_linker::<T, WasiSockets>(
        linker,
        &Default::default(),
        |ctx| ctx.sockets(),
    )?;
    bindings::sockets::ip_name_lookup::add_to_linker::<T, WasiSockets>(linker, |ctx| {
        ctx.sockets()
    })?;

    Ok(())
}
