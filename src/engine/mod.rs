use anyhow::{Context, Result};
use std::path::Path;
use wasmtime::component::Linker;
use wasmtime::{Config, Engine};
use wasmtime_wasi::p2::bindings::{cli, clocks, random};
use wasmtime_wasi::p2::bindings::sync::cli as sync_cli;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiView};
use wasmtime_wasi_http::WasiHttpView;

/// Configure a Wasmtime engine and linker with WASI support
///
/// # Errors
///
/// Returns an error if engine or linker configuration fails
pub fn configure_engine_and_linker<T>() -> Result<(Engine, Linker<T>)>
where
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
    clocks::monotonic_clock::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;
    cli::environment::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;
    random::random::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;
    sync_cli::stdin::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;
    sync_cli::stdout::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;
    sync_cli::stderr::add_to_linker::<_, Intercept<T>>(&mut linker, |ctx| ctx)?;

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
    use wasmtime_wasi::filesystem::{WasiFilesystem, WasiFilesystemView};
    use wasmtime_wasi::p2::bindings;
    use wasmtime_wasi::random::{WasiRandom, WasiRandomView};
    use wasmtime_wasi::sockets::{WasiSockets, WasiSocketsView};

    // Add CLI components (except environment, stdin, stdout, and stderr which we intercept)
    bindings::sync::cli::exit::add_to_linker::<T, WasiCli>(linker, &Default::default(), |ctx| {
        ctx.cli()
    })?;
    bindings::sync::cli::terminal_input::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;
    bindings::sync::cli::terminal_output::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;
    bindings::sync::cli::terminal_stdin::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;
    bindings::sync::cli::terminal_stdout::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;
    bindings::sync::cli::terminal_stderr::add_to_linker::<T, WasiCli>(linker, |ctx| ctx.cli())?;

    // No clock components to add here - wall_clock and monotonic_clock are intercepted

    // Add filesystem components (not intercepted for now due to complex trait requirements)
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

/// Build a WASI context for a given WASM component
pub fn build_wasi_ctx(wasm_path: &Path, args: &[String]) -> WasiCtx {
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
