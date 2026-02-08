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
