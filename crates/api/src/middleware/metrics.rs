use axum::{body::Body, http::Request, middleware::Next, response::Response};

use crate::state::METRICS;

pub async fn metrics(req: Request<Body>, next: Next) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let resp = next.run(req).await;
    let status = resp.status().as_u16();
    METRICS.record_http_request(&method, &path, status);
    resp
}
