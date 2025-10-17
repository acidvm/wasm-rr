use std::io::ErrorKind;

/// Check if a ciborium error is caused by reaching EOF.
/// Ciborium Error<T> enum has an Io(T) variant that wraps IO errors directly.
pub fn is_cbor_eof(err: &ciborium::de::Error<std::io::Error>) -> bool {
    matches!(err, ciborium::de::Error::Io(io_err) if io_err.kind() == ErrorKind::UnexpectedEof)
}
