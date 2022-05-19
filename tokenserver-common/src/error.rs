use std::fmt;

use http::StatusCode;
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
#[error("{context}")]
pub struct TokenserverError {
    pub status: &'static str,
    pub location: ErrorLocation,
    pub name: String,
    pub description: &'static str,
    pub http_status: StatusCode,
    /// For internal use only. Used to report any additional context behind an error to
    /// distinguish between similar errors in Sentry.
    pub context: String,
}

impl Default for TokenserverError {
    fn default() -> Self {
        Self {
            status: "error",
            location: ErrorLocation::default(),
            name: "".to_owned(),
            description: "Unauthorized",
            http_status: StatusCode::UNAUTHORIZED,
            context: "Unauthorized".to_owned(),
        }
    }
}

impl TokenserverError {
    pub fn invalid_generation() -> Self {
        Self {
            status: "invalid-generation",
            location: ErrorLocation::Body,
            context: "Invalid generation".to_owned(),
            ..Self::default()
        }
    }

    pub fn invalid_keys_changed_at() -> Self {
        Self {
            status: "invalid-keysChangedAt",
            location: ErrorLocation::Body,
            context: "Invalid keys_changed_at".to_owned(),
            ..Self::default()
        }
    }

    pub fn invalid_key_id(description: &'static str) -> Self {
        Self {
            status: "invalid-key-id",
            description,
            context: description.to_owned(),
            ..Self::default()
        }
    }

    pub fn invalid_credentials(description: &'static str) -> Self {
        Self {
            status: "invalid-credentials",
            location: ErrorLocation::Body,
            description,
            context: description.to_owned(),
            ..Self::default()
        }
    }

    pub fn invalid_client_state(description: &'static str) -> Self {
        Self {
            status: "invalid-client-state",
            description,
            name: "X-Client-State".to_owned(),
            context: description.to_owned(),
            ..Self::default()
        }
    }

    pub fn internal_error() -> Self {
        Self {
            status: "internal-error",
            location: ErrorLocation::Internal,
            description: "Server error",
            http_status: StatusCode::INTERNAL_SERVER_ERROR,
            context: "Internal error".to_owned(),
            ..Self::default()
        }
    }

    pub fn resource_unavailable() -> Self {
        Self {
            location: ErrorLocation::Body,
            description: "Resource is not available",
            http_status: StatusCode::SERVICE_UNAVAILABLE,
            context: "Resource is not available".to_owned(),
            ..Default::default()
        }
    }

    pub fn unsupported(description: &'static str, name: String) -> Self {
        Self {
            status: "error",
            location: ErrorLocation::Url,
            description,
            name,
            http_status: StatusCode::NOT_FOUND,
            context: description.to_owned(),
        }
    }

    pub fn unauthorized(description: &'static str) -> Self {
        Self {
            location: ErrorLocation::Body,
            description,
            context: description.to_owned(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ErrorLocation {
    Header,
    Url,
    Body,
    Internal,
}

impl Default for ErrorLocation {
    fn default() -> Self {
        Self::Header
    }
}

impl fmt::Display for ErrorLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header => write!(f, "header"),
            Self::Url => write!(f, "url"),
            Self::Body => write!(f, "body"),
            Self::Internal => write!(f, "internal"),
        }
    }
}

// impl Serialize for TokenserverError {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         ErrorResponse::from(self).serialize(serializer)
//     }
// }
