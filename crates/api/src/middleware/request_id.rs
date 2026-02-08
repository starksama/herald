use axum::{body::Body, http::Request, middleware::Next, response::Response};
use nanoid::nanoid;

use crate::state::RequestId;

pub async fn request_id(mut req: Request<Body>, next: Next) -> Response {
    let request_id = format!("req_{}", nanoid!(16));
    req.extensions_mut().insert(RequestId(request_id.clone()));
    let mut resp = next.run(req).await;
    if let Ok(value) = request_id.parse() {
        resp.headers_mut().insert("X-Request-Id", value);
    }
    resp
}
