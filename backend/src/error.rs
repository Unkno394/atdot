use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};

pub enum AppError {
    BadRequest(String),
    Auth(String),
    NotFound(String),
    Internal(String),
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound("not found".into()),
            _ => AppError::Internal(e.to_string()),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST,          m),
            AppError::Auth(m)       => (StatusCode::UNAUTHORIZED,         m),
            AppError::NotFound(m)   => (StatusCode::NOT_FOUND,            m),
            AppError::Internal(m)   => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}
