use assert_cmd::Command;
use quickcheck::{Arbitrary, Gen};
use quickcheck_macros::quickcheck;
use std::fs;
use std::io::Write;
use tempfile::TempDir;
use wasm_rr::{TraceEvent, TraceFile};

// Newtype wrappers to implement Arbitrary without violating orphan rules
#[derive(Debug, Clone, PartialEq, Eq)]
struct TestTraceEvent(TraceEvent);

#[derive(Debug, Clone, PartialEq, Eq)]
struct TestTraceFile(TraceFile);

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

impl Arbitrary for TestTraceEvent {
    fn arbitrary(g: &mut Gen) -> Self {
        let variant = u8::arbitrary(g) % 10;
        let event = match variant {
            0 => TraceEvent::ClockNow {
                seconds: u64::arbitrary(g),
                nanoseconds: u32::arbitrary(g) % 1_000_000_000,
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
                bytes: arbitrary_vec_limited(g, 1024),
            },
            8 => TraceEvent::RandomU64 {
                value: u64::arbitrary(g),
            },
            9 => TraceEvent::HttpResponse {
                request_method: arbitrary_string(g),
                request_url: arbitrary_string(g),
                request_headers: arbitrary_vec_limited(g, 20),
                status: u16::arbitrary(g) % 600,
                headers: arbitrary_vec_limited(g, 20),
                body: arbitrary_vec_limited(g, 1024),
            },
            _ => unreachable!(),
        };
        TestTraceEvent(event)
    }
}

impl Arbitrary for TestTraceFile {
    fn arbitrary(g: &mut Gen) -> Self {
        TestTraceFile(TraceFile {
            events: arbitrary_vec_limited::<TestTraceEvent>(g, 50)
                .into_iter()
                .map(|TestTraceEvent(e)| e)
                .collect(),
        })
    }
}

#[quickcheck]
fn roundtrip_json_to_cbor_to_json(test_trace: TestTraceFile) -> Result<bool, String> {
    let TestTraceFile(trace) = test_trace;
    let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

    // Write original trace as JSON
    let json_path = temp_dir.path().join("original.json");
    let json_file =
        fs::File::create(&json_path).map_err(|e| format!("Failed to create JSON file: {}", e))?;
    serde_json::to_writer_pretty(&json_file, &trace)
        .map_err(|e| format!("Failed to write JSON: {}", e))?;

    // Convert JSON to CBOR
    let cbor_path = temp_dir.path().join("converted.cbor");
    Command::cargo_bin("wasm-rr")
        .map_err(|e| format!("Failed to find binary: {}", e))?
        .arg("convert")
        .arg(&json_path)
        .arg(&cbor_path)
        .assert()
        .success();

    // Convert CBOR back to JSON
    let json2_path = temp_dir.path().join("roundtrip.json");
    Command::cargo_bin("wasm-rr")
        .map_err(|e| format!("Failed to find binary: {}", e))?
        .arg("convert")
        .arg(&cbor_path)
        .arg(&json2_path)
        .assert()
        .success();

    // Read the roundtrip JSON
    let json2_file =
        fs::File::open(&json2_path).map_err(|e| format!("Failed to open roundtrip JSON: {}", e))?;
    let trace2: TraceFile = serde_json::from_reader(&json2_file)
        .map_err(|e| format!("Failed to parse roundtrip JSON: {}", e))?;

    // Compare
    Ok(trace == trace2)
}

#[quickcheck]
fn roundtrip_cbor_to_json_to_cbor(test_trace: TestTraceFile) -> Result<bool, String> {
    let TestTraceFile(trace) = test_trace;
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
    Command::cargo_bin("wasm-rr")
        .map_err(|e| format!("Failed to find binary: {}", e))?
        .arg("convert")
        .arg(&cbor_path)
        .arg(&json_path)
        .assert()
        .success();

    // Convert JSON back to CBOR
    let cbor2_path = temp_dir.path().join("roundtrip.cbor");
    Command::cargo_bin("wasm-rr")
        .map_err(|e| format!("Failed to find binary: {}", e))?
        .arg("convert")
        .arg(&json_path)
        .arg(&cbor2_path)
        .assert()
        .success();

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
