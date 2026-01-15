use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

/// Test that file I/O can be recorded and replayed deterministically.
///
/// This test:
/// 1. Creates a temporary file with known content
/// 2. Compiles a simple WASM component that reads the file
/// 3. Records the execution
/// 4. Deletes the original file
/// 5. Replays the execution (should succeed using recorded data)
#[test]
#[ignore = "requires wasm component compilation"]
fn test_file_read_recording_replay() {
    // This test requires a pre-compiled WASM component
    // For now, we'll skip it if the component doesn't exist
    let wasm_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("wasm32-wasip2")
        .join("debug")
        .join("read_file.wasm");

    if !wasm_path.exists() {
        eprintln!("Skipping test: read_file.wasm not found at {:?}", wasm_path);
        eprintln!("Build it with: cargo build --target wasm32-wasip2 -p read_file");
        return;
    }

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let input_file = temp_dir.path().join("input.txt");
    let trace_file = temp_dir.path().join("trace.json");

    // Create test file
    let test_content = "Hello, this is test content for file I/O recording!";
    fs::write(&input_file, test_content).expect("Failed to write test file");

    // Get path to wasm-rr binary
    let wasm_rr = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("wasm-rr");

    // Record execution
    let record_output = Command::new(&wasm_rr)
        .args([
            "record",
            wasm_path.to_str().unwrap(),
            "-t",
            trace_file.to_str().unwrap(),
            "--",
            input_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run wasm-rr record");

    assert!(
        record_output.status.success(),
        "Recording failed: {}",
        String::from_utf8_lossy(&record_output.stderr)
    );

    // Verify the trace file was created
    assert!(
        trace_file.exists(),
        "Trace file should exist after recording"
    );

    // Delete the input file to prove replay uses recorded data
    fs::remove_file(&input_file).expect("Failed to delete input file");
    assert!(!input_file.exists(), "Input file should be deleted");

    // Replay execution (should succeed without the original file)
    let replay_output = Command::new(&wasm_rr)
        .args([
            "replay",
            wasm_path.to_str().unwrap(),
            trace_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run wasm-rr replay");

    assert!(
        replay_output.status.success(),
        "Replay failed: {}",
        String::from_utf8_lossy(&replay_output.stderr)
    );

    // Both outputs should be identical
    assert_eq!(
        record_output.stdout, replay_output.stdout,
        "Record and replay stdout should match"
    );
}

/// Test that trace events are properly serialized/deserialized
#[test]
fn test_trace_event_serialization() {
    use wasm_rr::trace::TraceEvent;

    let events = vec![
        TraceEvent::StreamRead {
            data: vec![1, 2, 3, 4, 5],
            eof: false,
        },
        TraceEvent::StreamRead {
            data: vec![],
            eof: true,
        },
        TraceEvent::FileRead {
            data: vec![72, 101, 108, 108, 111], // "Hello"
            eof: false,
        },
        TraceEvent::FileRead {
            data: vec![],
            eof: true,
        },
    ];

    for event in events {
        let json = serde_json::to_string(&event).expect("Failed to serialize");
        let deserialized: TraceEvent = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(event, deserialized);
    }
}
