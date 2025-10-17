use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::event::TraceEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceFormat {
    Json,
    Cbor,
}

impl TraceFormat {
    pub fn from_path_and_option(path: &Path, format_opt: Option<&str>) -> Result<Self> {
        if let Some(format_str) = format_opt {
            return match format_str {
                "json" => Ok(TraceFormat::Json),
                "cbor" => Ok(TraceFormat::Cbor),
                _ => bail!("unsupported format: {}", format_str),
            };
        }

        // Infer from file extension
        match path.extension().and_then(|s| s.to_str()) {
            Some("json") => Ok(TraceFormat::Json),
            Some("cbor") => Ok(TraceFormat::Cbor),
            Some(ext) => bail!("unsupported file extension: .{}", ext),
            None => bail!("cannot determine trace format: no file extension"),
        }
    }
}

/// A trace file containing multiple events
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct TraceFile {
    pub events: Vec<TraceEvent>,
}
