use quickcheck::{Arbitrary, Gen};
use quickcheck_macros::quickcheck;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// TraceEvent definition matching the one in main.rs
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
    MonotonicClockNow {
        nanoseconds: u64,
    },
    MonotonicClockResolution {
        nanoseconds: u64,
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
}

impl Arbitrary for TraceEvent {
    fn arbitrary(g: &mut Gen) -> Self {
        let variant = u8::arbitrary(g) % 10;
        match variant {
            0 => TraceEvent::ClockNow {
                seconds: u64::arbitrary(g),
                nanoseconds: u32::arbitrary(g) % 1_000_000_000, // Valid nanoseconds range
            },
            1 => TraceEvent::ClockResolution {
                seconds: u64::arbitrary(g),
                nanoseconds: u32::arbitrary(g) % 1_000_000_000,
            },
            2 => TraceEvent::MonotonicClockNow {
                nanoseconds: u64::arbitrary(g),
            },
            3 => TraceEvent::MonotonicClockResolution {
                nanoseconds: u64::arbitrary(g),
            },
            4 => TraceEvent::Environment {
                entries: arbitrary_vec_limited(g, 10),
            },
            5 => TraceEvent::Arguments {
                args: arbitrary_vec_limited(g, 10),
            },
            6 => TraceEvent::InitialCwd {
                path: Option::<String>::arbitrary(g),
            },
            7 => TraceEvent::RandomBytes {
                bytes: arbitrary_vec_limited(g, 1024), // Limit size for performance
            },
            8 => TraceEvent::RandomU64 {
                value: u64::arbitrary(g),
            },
            9 => TraceEvent::HttpResponse {
                request_method: arbitrary_string(g),
                request_url: arbitrary_string(g),
                request_headers: arbitrary_vec_limited(g, 20),
                status: u16::arbitrary(g) % 600, // Valid HTTP status codes
                headers: arbitrary_vec_limited(g, 20),
                body: arbitrary_vec_limited(g, 1024),
            },
            _ => unreachable!(),
        }
    }
}

/// Generate a limited-size vector to avoid overly large test cases
fn arbitrary_vec_limited<T: Arbitrary>(g: &mut Gen, max_size: usize) -> Vec<T> {
    let size = usize::arbitrary(g) % max_size.min(g.size());
    (0..size).map(|_| T::arbitrary(g)).collect()
}

/// Generate a reasonable-sized string
fn arbitrary_string(g: &mut Gen) -> String {
    let bytes: Vec<u8> = arbitrary_vec_limited(g, 100);
    // Filter to valid UTF-8 or use ASCII
    String::from_utf8(
        bytes
            .into_iter()
            .filter(|&b| (32..127).contains(&b))
            .collect(),
    )
    .unwrap_or_else(|_| String::from("test"))
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
struct TraceFile {
    events: Vec<TraceEvent>,
}

impl Arbitrary for TraceFile {
    fn arbitrary(g: &mut Gen) -> Self {
        TraceFile {
            events: arbitrary_vec_limited(g, 50), // Limit to 50 events
        }
    }
}

/// Run the convert command using the binary
fn run_convert(input: &PathBuf, output: &PathBuf) -> Result<(), String> {
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_wasm-rr"))
        .arg("convert")
        .arg(input)
        .arg(output)
        .status()
        .map_err(|e| format!("Failed to run convert: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("Convert command failed with status: {}", status))
    }
}

#[quickcheck]
fn roundtrip_json_to_cbor_to_json(trace: TraceFile) -> Result<bool, String> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // Write original trace as JSON
    let json_path = temp_dir.path().join("original.json");
    let json_file =
        fs::File::create(&json_path).map_err(|e| format!("Failed to create JSON file: {}", e))?;
    serde_json::to_writer_pretty(&json_file, &trace)
        .map_err(|e| format!("Failed to write JSON: {}", e))?;

    // Convert JSON to CBOR
    let cbor_path = temp_dir.path().join("converted.cbor");
    run_convert(&json_path, &cbor_path)?;

    // Convert CBOR back to JSON
    let json2_path = temp_dir.path().join("roundtrip.json");
    run_convert(&cbor_path, &json2_path)?;

    // Read the roundtrip JSON
    let json2_file =
        fs::File::open(&json2_path).map_err(|e| format!("Failed to open roundtrip JSON: {}", e))?;
    let trace2: TraceFile = serde_json::from_reader(&json2_file)
        .map_err(|e| format!("Failed to parse roundtrip JSON: {}", e))?;

    // Compare
    Ok(trace == trace2)
}

#[quickcheck]
fn roundtrip_cbor_to_json_to_cbor(trace: TraceFile) -> Result<bool, String> {
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // Write original trace as CBOR
    let cbor_path = temp_dir.path().join("original.cbor");
    let mut cbor_file =
        fs::File::create(&cbor_path).map_err(|e| format!("Failed to create CBOR file: {}", e))?;
    for event in &trace.events {
        ciborium::into_writer(event, &mut cbor_file)
            .map_err(|e| format!("Failed to write CBOR: {}", e))?;
    }
    cbor_file
        .flush()
        .map_err(|e| format!("Failed to flush CBOR: {}", e))?;

    // Convert CBOR to JSON
    let json_path = temp_dir.path().join("converted.json");
    run_convert(&cbor_path, &json_path)?;

    // Convert JSON back to CBOR
    let cbor2_path = temp_dir.path().join("roundtrip.cbor");
    run_convert(&json_path, &cbor2_path)?;

    // Read the roundtrip CBOR
    let cbor2_file =
        fs::File::open(&cbor2_path).map_err(|e| format!("Failed to open roundtrip CBOR: {}", e))?;
    let mut reader = std::io::BufReader::new(cbor2_file);
    let mut events2 = Vec::new();
    loop {
        match ciborium::from_reader::<TraceEvent, _>(&mut reader) {
            Ok(event) => events2.push(event),
            Err(e) => {
                // Check for EOF
                if matches!(e, ciborium::de::Error::Io(ref io_err) if io_err.kind() == std::io::ErrorKind::UnexpectedEof)
                {
                    break;
                }
                return Err(format!("Failed to read CBOR: {}", e));
            }
        }
    }
    let trace2 = TraceFile { events: events2 };

    // Compare
    Ok(trace == trace2)
}
