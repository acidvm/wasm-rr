use serde::{Deserialize, Serialize};

/// Helper module for hex encoding/decoding Vec<u8>
mod hex_serde {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        hex::decode(&s).map_err(serde::de::Error::custom)
    }
}

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
        #[serde(with = "hex_serde")]
        bytes: Vec<u8>,
    },
    RandomU64 {
        value: u64,
    },
    /// Legacy read marker - kept for backward compatibility
    Read,
    /// Stream read with actual data (for wasi:io/streams InputStream)
    /// Empty data with eof=true indicates stream closed
    StreamRead {
        #[serde(with = "hex_serde")]
        data: Vec<u8>,
        #[serde(default)]
        eof: bool,
    },
    /// File read with actual data (for wasi:filesystem/types Descriptor::read)
    FileRead {
        #[serde(with = "hex_serde")]
        data: Vec<u8>,
        eof: bool,
    },
    HttpResponse {
        request_method: String,
        request_url: String,
        request_headers: Vec<(String, String)>,
        status: u16,
        headers: Vec<(String, String)>,
        #[serde(with = "hex_serde")]
        body: Vec<u8>,
    },
}
