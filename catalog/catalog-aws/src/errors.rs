//! Narrow error taxonomy for catalog-aws operations.
//!
//! Context-rich conversion from AWS SDK errors to catalog-specific error types.

use std::fmt;

/// Catalog-specific error taxonomy.
#[derive(Debug)]
pub enum CatalogError {
    /// Requested item was not found.
    NotFound {
        kind: String,
        key: String,
        source: Option<anyhow::Error>,
    },
    /// Invalid input (e.g. malformed version, missing required field).
    InvalidInput {
        message: String,
        source: Option<anyhow::Error>,
    },
    /// Storage/backend failure (DynamoDB, S3, etc.).
    Storage {
        operation: String,
        source: anyhow::Error,
    },
    /// Serialization/deserialization failure.
    Serialization {
        context: String,
        source: anyhow::Error,
    },
    /// Conflict (e.g. conditional write failed, version mismatch).
    Conflict {
        message: String,
        source: Option<anyhow::Error>,
    },
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatalogError::NotFound { kind, key, .. } => {
                write!(f, "not found: {} {}", kind, key)
            }
            CatalogError::InvalidInput { message, .. } => {
                write!(f, "invalid input: {}", message)
            }
            CatalogError::Storage { operation, source } => {
                write!(f, "storage error during {}: {}", operation, source)
            }
            CatalogError::Serialization { context, source } => {
                write!(f, "serialization error ({}): {}", context, source)
            }
            CatalogError::Conflict { message, .. } => {
                write!(f, "conflict: {}", message)
            }
        }
    }
}

impl std::error::Error for CatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CatalogError::NotFound { source, .. }
            | CatalogError::InvalidInput { source, .. }
            | CatalogError::Conflict { source, .. } => source
                .as_ref()
                .map(|e| e.as_ref() as &(dyn std::error::Error + 'static)),
            CatalogError::Storage { source, .. } | CatalogError::Serialization { source, .. } => {
                Some(source.as_ref() as &(dyn std::error::Error + 'static))
            }
        }
    }
}
