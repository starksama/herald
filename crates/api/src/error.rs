use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    pub request_id: String,
}

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized,
    Forbidden(String),
    NotFound(String),
    RateLimited,
    Internal,
}

#[derive(Debug)]
pub struct ApiError {
    pub error: AppError,
    pub request_id: String,
}

impl AppError {
    pub fn with_request_id(self, request_id: &str) -> ApiError {
        ApiError {
            error: self,
            request_id: request_id.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match self.error {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "invalid_request", msg),
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "Invalid API key".to_string(),
            ),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, "forbidden", msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            AppError::RateLimited => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "Too many requests".to_string(),
            ),
            AppError::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "Unexpected error".to_string(),
            ),
        };

        (
            status,
            Json(ErrorResponse {
                error: ErrorBody {
                    code: code.to_string(),
                    message,
                    request_id: self.request_id,
                },
            }),
        )
            .into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use axum::response::IntoResponse;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    #[test]
    fn test_with_request_id() {
        let err = AppError::Internal.with_request_id("req_123");
        assert_eq!(err.request_id, "req_123");
    }

    #[test]
    fn test_with_request_id_empty() {
        let err = AppError::Unauthorized.with_request_id("");
        assert_eq!(err.request_id, "");
    }

    #[test]
    fn test_bad_request_response() {
        rt().block_on(async {
            let err = AppError::BadRequest("missing field".to_string()).with_request_id("req_001");
            let response = err.into_response();

            assert_eq!(response.status(), StatusCode::BAD_REQUEST);

            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

            assert_eq!(json["error"]["code"], "invalid_request");
            assert_eq!(json["error"]["message"], "missing field");
            assert_eq!(json["error"]["request_id"], "req_001");
        });
    }

    #[test]
    fn test_unauthorized_response() {
        rt().block_on(async {
            let err = AppError::Unauthorized.with_request_id("req_002");
            let response = err.into_response();

            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

            assert_eq!(json["error"]["code"], "unauthorized");
            assert_eq!(json["error"]["message"], "Invalid API key");
        });
    }

    #[test]
    fn test_forbidden_response() {
        rt().block_on(async {
            let err = AppError::Forbidden("no access".to_string()).with_request_id("req_003");
            let response = err.into_response();

            assert_eq!(response.status(), StatusCode::FORBIDDEN);

            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

            assert_eq!(json["error"]["code"], "forbidden");
            assert_eq!(json["error"]["message"], "no access");
        });
    }

    #[test]
    fn test_not_found_response() {
        rt().block_on(async {
            let err = AppError::NotFound("channel xyz".to_string()).with_request_id("req_004");
            let response = err.into_response();

            assert_eq!(response.status(), StatusCode::NOT_FOUND);

            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

            assert_eq!(json["error"]["code"], "not_found");
            assert_eq!(json["error"]["message"], "channel xyz");
        });
    }

    #[test]
    fn test_rate_limited_response() {
        rt().block_on(async {
            let err = AppError::RateLimited.with_request_id("req_005");
            let response = err.into_response();

            assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

            assert_eq!(json["error"]["code"], "rate_limited");
            assert_eq!(json["error"]["message"], "Too many requests");
        });
    }

    #[test]
    fn test_internal_error_response() {
        rt().block_on(async {
            let err = AppError::Internal.with_request_id("req_006");
            let response = err.into_response();

            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

            let body = to_bytes(response.into_body(), 1024).await.unwrap();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();

            assert_eq!(json["error"]["code"], "internal_error");
            assert_eq!(json["error"]["message"], "Unexpected error");
        });
    }
}
