use clap::{Parser, Subcommand};
use clap_markdown::help_markdown;
use std::path::PathBuf;

/// A deterministic record-replay tool for WebAssembly components
#[derive(Parser, Debug)]
#[command(author, version, about, propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Command,
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

fn main() {
    // Print header
    println!("# wasm-rr CLI Reference");
    println!();
    println!("This page contains the auto-generated reference documentation for the `wasm-rr` command-line interface.");
    println!();

    // Generate and print the markdown using the type parameter
    println!("{}", help_markdown::<Cli>());
}
