//! Browsers download the people-facing reads and the UI's assets compressed; a large listing is a tenth
//! of its size on the wire.
//!
//! Only reads by people and UI assets are compressed. Sign-in responses carry the session's CSRF token, and
//! a compressed response that holds a secret beside text a requester chose can leak the secret through its
//! size, so they, writes, and the location API stay uncompressed.

use axum::extract::Request;
use axum::http::{Method, header};
use axum::middleware::Next;
use axum::response::Response;

pub(crate) async fn compress_reads_only(mut request: Request, next: Next) -> Response {
    let path = request.uri().path();
    let read = path.starts_with("/api/control/")
        || path.starts_with("/api/admin/")
        || !path.starts_with("/api/");
    if request.method() != Method::GET || !read {
        request.headers_mut().remove(header::ACCEPT_ENCODING);
    }
    next.run(request).await
}
