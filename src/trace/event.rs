use serde::{Deserialize, Serialize};

/// A single trace event recorded during execution
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "call", rename_all = "snake_case")]
pub enum TraceEvent {
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
    // Filesystem operations (non-deterministic for now)
    DescriptorRead {
        fd: u32,
        len: u64,
        // We don't store the actual data yet, just log the operation
    },
    DescriptorWrite {
        fd: u32,
        len: u64,
        // We don't store the actual data yet, just log the operation
    },
    DescriptorSeek {
        fd: u32,
        offset: i64,
        whence: String,
    },
    DescriptorOpenAt {
        fd: u32,
        path: String,
        flags: Vec<String>,
    },
}
