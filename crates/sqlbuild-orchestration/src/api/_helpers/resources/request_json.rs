//! JSON request bodies that may arrive gzip-encoded, decoded within a decompressed-size bound.

use std::io::{BufReader, Read};

use axum::http::{HeaderMap, StatusCode, header};
use flate2::read::MultiGzDecoder;
use serde::de::DeserializeOwned;

use crate::api::constants::{GZIP_ENCODING, IDENTITY_ENCODING, JSON_MEDIA_TYPE};
use crate::api::errors::ApiError;
use crate::api::models::BoundedReader;

/// `body` as JSON after its declared encoding; gzip streams into the parser and stops past `limit` bytes.
pub(crate) fn request_json<T: DeserializeOwned>(
    headers: &HeaderMap,
    body: &[u8],
    limit: usize,
) -> Result<T, ApiError> {
    if !json_content(headers) {
        return Err(rejection(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "expected an application/json body".into(),
        ));
    }
    match encodings(headers)?.as_slice() {
        [] => serde_json::from_slice(body).map_err(|error| parse_rejection(&error)),
        [encoding] if encoding == GZIP_ENCODING => {
            let mut reader = BoundedReader {
                inner: MultiGzDecoder::new(body),
                remaining: limit,
                exceeded: false,
            };
            let parsed: Result<T, serde_json::Error> =
                serde_json::from_reader(BufReader::new(&mut reader));
            match parsed {
                Ok(value) => Ok(value),
                Err(_) if reader.exceeded => Err(rejection(
                    StatusCode::PAYLOAD_TOO_LARGE,
                    format!("decompressed body exceeds {limit} bytes"),
                )),
                Err(error) if error.is_io() => Err(rejection(
                    StatusCode::BAD_REQUEST,
                    "request body is not valid gzip".into(),
                )),
                Err(error) => Err(parse_rejection(&error)),
            }
        }
        _ => Err(rejection(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "only gzip content encoding is accepted".into(),
        )),
    }
}

impl<R: Read> Read for BoundedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let window = buffer.len().min(self.remaining.saturating_add(1));
        let read = self.inner.read(&mut buffer[..window])?;
        if read > self.remaining {
            self.exceeded = true;
            return Err(std::io::Error::other("decompressed body exceeds its bound"));
        }
        self.remaining -= read;
        Ok(read)
    }
}

fn json_content(headers: &HeaderMap) -> bool {
    let Some(value) = headers.get(header::CONTENT_TYPE) else {
        return false;
    };
    let Ok(text) = value.to_str() else {
        return false;
    };
    let essence: String = text
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    essence == JSON_MEDIA_TYPE
        || (essence.starts_with("application/") && essence.ends_with("+json"))
}

/// Declared encodings other than identity, in order; an unreadable header is refused.
fn encodings(headers: &HeaderMap) -> Result<Vec<String>, ApiError> {
    let mut encodings: Vec<String> = Vec::new();
    for value in headers.get_all(header::CONTENT_ENCODING) {
        let text = value.to_str().map_err(|_| {
            rejection(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "only gzip content encoding is accepted".into(),
            )
        })?;
        encodings.extend(
            text.split(',')
                .map(|encoding| encoding.trim().to_ascii_lowercase())
                .filter(|encoding| !encoding.is_empty() && encoding != IDENTITY_ENCODING),
        );
    }
    Ok(encodings)
}

/// The statuses axum's JSON extractor uses: malformed JSON is 400, a wrong shape is 422.
fn parse_rejection(error: &serde_json::Error) -> ApiError {
    if error.is_data() {
        rejection(
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("Failed to deserialize the JSON body into the target type: {error}"),
        )
    } else {
        rejection(
            StatusCode::BAD_REQUEST,
            format!("Failed to parse the request body as JSON: {error}"),
        )
    }
}

fn rejection(status: StatusCode, message: String) -> ApiError {
    ApiError {
        status,
        message,
        decision_id: None,
    }
}
