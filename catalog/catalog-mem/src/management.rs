//! [`CatalogManagement`](catalog_trait::CatalogManagement) for [`MemCatalog`](crate::MemCatalog).
//!
//! ## Promote
//!
//! Updates the entry’s [`Metadata::track`](catalog_trait::types::Metadata::track) and optionally
//! [`Metadata::version`](catalog_trait::types::Metadata::version) in place. The [`CatalogRef::id`](catalog_trait::types::CatalogRef::id)
//! is unchanged. If `track` changes, the secondary index is updated so `list` / `Latest` use the new
//! logical `(kind, name, track)` bucket. Promoting to a `(track, version)` already held by another
//! entry (same kind and name) returns an error.
//!
//! ## Deprecate
//!
//! Sets [`Metadata::deprecated`](catalog_trait::types::Metadata::deprecated) and
//! [`Metadata::deprecated_message`](catalog_trait::types::Metadata::deprecated_message). Deprecated rows
//! remain visible in [`CatalogRead::list`](catalog_trait::CatalogRead::list) and participate in
//! [`VersionSelector::Latest`](catalog_trait::types::VersionSelector::Latest) (same ordering as before).
//!
//! ## Yank
//!
//! Marks the row as **yanked**: it is excluded from [`CatalogRead::list`](catalog_trait::CatalogRead::list),
//! [`CatalogRead::get`](catalog_trait::CatalogRead::get), and `Latest` resolution. Direct
//! [`CatalogRead::download_*`](catalog_trait::CatalogRead::download_provider) and attachment access by id
//! also fail with a clear error.

use async_trait::async_trait;
use catalog_trait::types::{CatalogKind, CatalogRef};
use catalog_trait::CatalogManagement;

use crate::MemCatalog;

#[async_trait]
impl CatalogManagement for MemCatalog {
    async fn promote(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        track: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        self.store.promote(kind, reference, track, version)
    }

    async fn deprecate(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        reason: &str,
    ) -> anyhow::Result<()> {
        self.store.deprecate(kind, reference, reason)
    }

    async fn yank(&self, kind: CatalogKind, reference: &CatalogRef) -> anyhow::Result<()> {
        self.store.yank(kind, reference)
    }
}
