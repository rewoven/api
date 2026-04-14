use axum::{http::StatusCode, response::IntoResponse, Json};

use crate::models::ErrorResponse;

pub enum AppError {
    Db(String),
    NotFound(String),
    Pool(r2d2::Error),
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<r2d2::Error> for AppError {
    fn from(e: r2d2::Error) -> Self {
        AppError::Pool(e)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            AppError::Db(msg) => {
                tracing::error!("Database error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal database error".to_string())
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Pool(e) => {
                tracing::error!("Connection pool error: {}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Service temporarily unavailable".to_string())
            }
        };
        (status, Json(ErrorResponse { error: message, status: status.as_u16() })).into_response()
    }
}
