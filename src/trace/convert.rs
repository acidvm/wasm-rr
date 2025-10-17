use anyhow::{Context, Result};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

use super::event::TraceEvent;
use super::format::{TraceFile, TraceFormat};
use crate::util::cbor::is_cbor_eof;

pub fn convert(
    input: &Path,
    output: &Path,
    input_format: TraceFormat,
    output_format: TraceFormat,
) -> Result<()> {
    let input_file = File::open(input)
        .with_context(|| format!("failed to open input trace file at {}", input.display()))?;
    let reader = BufReader::new(input_file);

    let events: Vec<TraceEvent> = match input_format {
        TraceFormat::Json => {
            let TraceFile { events } = serde_json::from_reader(reader).with_context(|| {
                format!("failed to parse JSON trace file at {}", input.display())
            })?;
            events
        }
        TraceFormat::Cbor => {
            let mut events = Vec::new();
            let mut reader = reader;
            loop {
                match ciborium::from_reader::<TraceEvent, _>(&mut reader) {
                    Ok(event) => events.push(event),
                    Err(e) if is_cbor_eof(&e) => break,
                    Err(e) => {
                        return Err(anyhow::Error::msg(format!("{}", e))).with_context(|| {
                            format!("failed to parse CBOR trace file at {}", input.display())
                        });
                    }
                }
            }
            events
        }
    };

    let output_file = File::create(output)
        .with_context(|| format!("failed to create output trace file at {}", output.display()))?;

    match output_format {
        TraceFormat::Json => {
            let trace = TraceFile { events };
            serde_json::to_writer_pretty(output_file, &trace).with_context(|| {
                format!("failed to write JSON trace file at {}", output.display())
            })?;
        }
        TraceFormat::Cbor => {
            let mut writer = BufWriter::new(output_file);
            for event in events {
                ciborium::into_writer(&event, &mut writer).with_context(|| {
                    format!("failed to write CBOR trace file at {}", output.display())
                })?;
            }
            writer.flush().with_context(|| {
                format!("failed to flush CBOR trace file at {}", output.display())
            })?;
        }
    }

    Ok(())
}
