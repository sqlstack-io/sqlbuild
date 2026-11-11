//! Only people's reads and UI assets are compressed, never a response holding the CSRF token.

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
