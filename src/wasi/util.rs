use anyhow::anyhow;
use wasmtime_wasi_http::HttpError;

/// Sort HTTP headers for deterministic comparison
///
/// # Errors
///
/// Returns an error if header values contain invalid UTF-8
pub fn sorted_headers(
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

/// Build a header map from sorted key-value pairs
///
/// # Errors
///
/// Returns an error if header names or values are invalid
pub fn header_map_from_pairs(
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
