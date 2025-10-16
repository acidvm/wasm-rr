use std::collections::VecDeque;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

use anyhow::anyhow;
use anyhow::Context;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use wasmtime::component::ResourceTable;
use wasmtime_wasi::p2::bindings::{cli, clocks, random};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};
use wasmtime_wasi_http::types::{
    HostFutureIncomingResponse, IncomingResponse, OutgoingRequestConfig,
};
use wasmtime_wasi_http::{HttpError, WasiHttpCtx, WasiHttpView};

use crate::{Result, TraceEvent, TraceFile};

pub struct Playback {
    events: VecDeque<TraceEvent>,
}

impl Playback {
    pub fn from_file(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("failed to open trace file at {}", path.display()))?;
        let TraceFile { events } = serde_json::from_reader(file)
            .with_context(|| format!("failed to parse trace file at {}", path.display()))?;
        Ok(Self {
            events: events.into(),
        })
    }

    pub fn next_event(&mut self) -> Result<TraceEvent> {
        self.events.pop_front().ok_or(anyhow!("trace exhausted"))
    }

    pub fn next_now(&mut self) -> Result<clocks::wall_clock::Datetime> {
        match self.next_event()? {
            TraceEvent::ClockNow {
                seconds,
                nanoseconds,
            } => Ok(clocks::wall_clock::Datetime {
                seconds,
                nanoseconds,
            }),
            other => Err(anyhow!(
                "expected next clock event to be 'now', got {:?}",
                other
            )),
        }
    }

    pub fn next_resolution(&mut self) -> Result<clocks::wall_clock::Datetime> {
        match self.next_event()? {
            TraceEvent::ClockResolution {
                seconds,
                nanoseconds,
            } => Ok(clocks::wall_clock::Datetime {
                seconds,
                nanoseconds,
            }),
            other => Err(anyhow!(
                "expected next clock event to be 'resolution', got {:?}",
                other
            )),
        }
    }

    pub fn next_environment(&mut self) -> Result<Vec<(String, String)>> {
        match self.next_event()? {
            TraceEvent::Environment { entries } => Ok(entries),
            other => Err(anyhow!("expected next environment event, got {:?}", other)),
        }
    }

    pub fn next_arguments(&mut self) -> Result<Vec<String>> {
        match self.next_event()? {
            TraceEvent::Arguments { args } => Ok(args),
            other => Err(anyhow!("expected next arguments event, got {:?}", other)),
        }
    }

    pub fn next_initial_cwd(&mut self) -> Result<Option<String>> {
        match self.next_event()? {
            TraceEvent::InitialCwd { path } => Ok(path),
            other => Err(anyhow!("expected next initial_cwd event, got {:?}", other)),
        }
    }

    pub fn next_random_bytes(&mut self, expected_len: u64) -> Result<Vec<u8>> {
        match self.next_event()? {
            TraceEvent::RandomBytes { bytes } => {
                if bytes.len() as u64 != expected_len {
                    return Err(anyhow!(
                        "random bytes length mismatch: expected {}, got {}",
                        expected_len,
                        bytes.len()
                    ));
                }
                Ok(bytes)
            }
            other => Err(anyhow!("expected next random_bytes event, got {:?}", other)),
        }
    }

    pub fn next_random_u64(&mut self) -> Result<u64> {
        match self.next_event()? {
            TraceEvent::RandomU64 { value } => Ok(value),
            other => Err(anyhow!("expected next random_u64 event, got {:?}", other)),
        }
    }

    fn next_http_response(&mut self) -> Result<(RecordedHttpRequest, RecordedHttpResponse)> {
        match self.next_event()? {
            TraceEvent::HttpResponse {
                request_method,
                request_url,
                request_headers,
                status,
                headers,
                body,
            } => Ok((
                RecordedHttpRequest {
                    method: request_method,
                    url: request_url,
                    headers: request_headers,
                },
                RecordedHttpResponse {
                    status,
                    headers,
                    body,
                },
            )),
            other => Err(anyhow!(
                "expected next http_response event, got {:?}",
                other
            )),
        }
    }

    pub fn finish(self) -> Result<()> {
        if self.events.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(
                "trace contains unused events: {:?}",
                self.events.into_iter().collect::<Vec<_>>()
            ))
        }
    }
}

struct RecordedHttpRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
}

struct RecordedHttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

pub struct CtxPlayback {
    table: ResourceTable,
    wasi: WasiCtx,
    http: WasiHttpCtx,
    playback: Playback,
}

impl CtxPlayback {
    pub fn new(wasi: WasiCtx, http: WasiHttpCtx, playback: Playback) -> Self {
        Self {
            table: ResourceTable::new(),
            wasi,
            http,
            playback,
        }
    }

    pub fn into_playback(self) -> Playback {
        self.playback
    }
}

impl WasiView for CtxPlayback {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl WasiHttpView for CtxPlayback {
    fn ctx(&mut self) -> &mut WasiHttpCtx {
        &mut self.http
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn send_request(
        &mut self,
        request: hyper::Request<wasmtime_wasi_http::body::HyperOutgoingBody>,
        _config: OutgoingRequestConfig,
    ) -> wasmtime_wasi_http::HttpResult<HostFutureIncomingResponse> {
        let method = request.method().to_string();
        let url = request.uri().to_string();
        let actual_headers = sorted_headers(request.headers())?;

        let (expected_request, recorded_response) = self
            .playback
            .next_http_response()
            .map_err(HttpError::trap)?;

        if method != expected_request.method || url != expected_request.url {
            return Err(HttpError::trap(anyhow!(
                "http request mismatch: expected {} {}, got {method} {url}",
                expected_request.method,
                expected_request.url
            )));
        }

        if actual_headers != expected_request.headers {
            return Err(HttpError::trap(anyhow!(
                "http request headers mismatch for {method} {url}"
            )));
        }

        let RecordedHttpResponse {
            status,
            headers,
            body,
        } = recorded_response;

        let mut builder = hyper::Response::builder().status(status);
        *builder
            .headers_mut()
            .ok_or_else(|| HttpError::trap(anyhow!("failed to access response headers")))? =
            header_map_from_pairs(&headers)?;

        // Full<Bytes> is infallible, but we need to convert the error type to match the expected signature
        let boxed_body = Full::new(Bytes::from(body))
            .map_err(|e: std::convert::Infallible| match e {})
            .boxed();

        let response = builder.body(boxed_body).map_err(HttpError::trap)?;

        let incoming = IncomingResponse {
            resp: response,
            worker: None,
            between_bytes_timeout: Duration::from_secs(600),
        };

        Ok(HostFutureIncomingResponse::ready(Ok(Ok(incoming))))
    }
}

impl clocks::wall_clock::Host for CtxPlayback {
    fn now(&mut self) -> std::result::Result<clocks::wall_clock::Datetime, anyhow::Error> {
        self.playback.next_now()
    }

    fn resolution(&mut self) -> std::result::Result<clocks::wall_clock::Datetime, anyhow::Error> {
        self.playback.next_resolution()
    }
}

impl cli::environment::Host for CtxPlayback {
    fn get_environment(&mut self) -> anyhow::Result<Vec<(String, String)>> {
        self.playback.next_environment()
    }

    fn get_arguments(&mut self) -> anyhow::Result<Vec<String>> {
        self.playback.next_arguments()
    }

    fn initial_cwd(&mut self) -> anyhow::Result<Option<String>> {
        self.playback.next_initial_cwd()
    }
}

impl random::random::Host for CtxPlayback {
    fn get_random_bytes(&mut self, len: u64) -> anyhow::Result<Vec<u8>> {
        self.playback.next_random_bytes(len)
    }

    fn get_random_u64(&mut self) -> anyhow::Result<u64> {
        self.playback.next_random_u64()
    }
}

fn sorted_headers(
    headers: &hyper::HeaderMap,
) -> wasmtime_wasi_http::HttpResult<Vec<(String, String)>> {
    let mut pairs = Vec::new();
    for (name, value) in headers.iter() {
        let value = value
            .to_str()
            .map_err(|err| HttpError::trap(anyhow!("invalid header value for {}: {err}", name)))?;
        pairs.push((name.as_str().to_string(), value.to_string()));
    }
    pairs.sort();
    Ok(pairs)
}

fn header_map_from_pairs(
    pairs: &[(String, String)],
) -> wasmtime_wasi_http::HttpResult<hyper::HeaderMap> {
    let mut map = hyper::HeaderMap::new();
    for (name, value) in pairs {
        let header_name = hyper::header::HeaderName::from_bytes(name.as_bytes())
            .map_err(|err| HttpError::trap(anyhow!("invalid header name {name}: {err}")))?;
        let header_value = hyper::header::HeaderValue::from_str(value)
            .map_err(|err| HttpError::trap(anyhow!("invalid header value for {name}: {err}")))?;
        map.append(header_name, header_value);
    }
    Ok(map)
}
