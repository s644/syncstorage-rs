use std::{error::Error, fmt};

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use backtrace::Backtrace;
use serde::{
    ser::{SerializeMap, Serializer},
    Serialize,
};
use syncstorage_common::from_error;
use syncstorage_db_common::error::DbError;
use thiserror::Error;
use tokenserver_common::error::{ErrorLocation, TokenserverError};

#[derive(Debug)]
pub struct ApiError {
    kind: ApiErrorKind,
    pub(crate) status: StatusCode,
    backtrace: Backtrace,
}

impl Error for ApiError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.kind.source()
    }
}

#[derive(Debug, Error)]
enum ApiErrorKind {
    #[error("{0}")]
    Application(TokenserverError),
    #[error("Database error: {0}")]
    Database(DbError),
}

impl From<ApiErrorKind> for ApiError {
    fn from(kind: ApiErrorKind) -> Self {
        match kind {
            ApiErrorKind::Application(ref error) => Self {
                status: error.http_status,
                kind,
                backtrace: Backtrace::new(),
            },
            ApiErrorKind::Database(ref error) => Self {
                backtrace: error.backtrace.clone(),
                status: error.status,
                kind,
            },
        }
    }
}

impl ResponseError for ApiError {
    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status).json(self)
    }

    fn status_code(&self) -> StatusCode {
        self.status
    }
}

#[derive(Debug)]
pub struct ErrorResponse {
    status: &'static str,
    errors: [ErrorInstance; 1],
}

#[derive(Debug)]
struct ErrorInstance {
    location: ErrorLocation,
    name: String,
    description: &'static str,
}

impl From<&ApiError> for ErrorResponse {
    fn from(error: &ApiError) -> Self {
        match &error.kind {
            ApiErrorKind::Application(error) => error.into(),
            ApiErrorKind::Database(db_error) => (&TokenserverError {
                description: "Database error",
                http_status: db_error.status,
                context: db_error.to_string(),
                ..Default::default()
            })
                .into(),
        }
    }
}

impl From<&TokenserverError> for ErrorResponse {
    fn from(error: &TokenserverError) -> Self {
        Self {
            status: error.status,
            errors: [ErrorInstance {
                location: error.location,
                name: error.name.clone(),
                description: error.description,
            }],
        }
    }
}

impl Serialize for ErrorInstance {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("location", &self.location.to_string())?;
        map.serialize_entry("name", &self.name)?;
        map.serialize_entry("description", &self.description)?;
        map.end()
    }
}

impl Serialize for ErrorResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("status", &self.status)?;
        map.serialize_entry("errors", &self.errors)?;
        map.end()
    }
}

impl Serialize for ApiError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ErrorResponse::from(self).serialize(serializer)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            serde_json::to_string(self).map_err(|_| fmt::Error)?
        )
    }
}

from_error!(DbError, ApiError, ApiErrorKind::Database);
from_error!(TokenserverError, ApiError, ApiErrorKind::Application);
