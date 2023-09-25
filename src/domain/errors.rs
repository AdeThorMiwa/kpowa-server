use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

pub enum DatabaseError {
    ServerError,
}

pub enum ApiError {
    InvalidInviteCode,
    ServerError,
    AuthenticationError,
}

impl From<DatabaseError> for ApiError {
    fn from(value: DatabaseError) -> Self {
        match value {
            DatabaseError::ServerError => Self::ServerError,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            Self::InvalidInviteCode => (StatusCode::BAD_REQUEST, "Invalid invite code"),
            Self::ServerError => (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong"),
            Self::AuthenticationError => (StatusCode::UNAUTHORIZED, "Authentication failed"),
        };

        let body = Json(json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}

#[derive(Debug)]
pub enum JWTError {
    GenerationFailed(jsonwebtoken::errors::ErrorKind),
    DecodeFailed(jsonwebtoken::errors::ErrorKind),
}

impl From<JWTError> for ApiError {
    fn from(_value: JWTError) -> Self {
        Self::AuthenticationError
    }
}
