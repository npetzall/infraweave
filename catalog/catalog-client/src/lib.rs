//! Convenience helpers for [`catalog_trait::Catalog`] consumers.
//!
//! - [`materialize_content`] turns a [`catalog_trait::read::ContentSource`] into owned bytes
//!   (filesystem read or HTTP GET for URLs).
//! - [`CatalogClient`] wraps a [`catalog_trait::Catalog`], implements that trait by delegating to the
//!   inner catalog, and normalizes [`catalog_trait::read::ContentSource`] from `download_*` to
//!   in-memory bytes. Callers that need a [`Vec<u8>`] can use [`materialize_content`] on the
//!   returned [`catalog_trait::read::ContentSource`].

#![forbid(unsafe_code)]

mod client;
mod content;

pub use catalog_trait;
pub use client::CatalogClient;
pub use content::materialize_content;
