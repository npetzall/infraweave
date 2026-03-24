use async_trait::async_trait;

use crate::types::{CatalogKind, CatalogRef};

#[async_trait]
pub trait CatalogManagement: Send + Sync {
    //
    // Unified management entrypoints
    //

    /// Promote an existing catalog entry to a new track/version state.
    ///
    /// Implementors can override this to handle all promote operations in one place.
    async fn promote(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        track: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()>;

    /// Mark an existing catalog entry as deprecated with an explicit reason.
    ///
    /// Implementors can override this to handle all deprecate operations in one place.
    async fn deprecate(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        reason: &str,
    ) -> anyhow::Result<()>;

    /// Yank (disable) an existing catalog entry from availability.
    ///
    /// Implementors can override this to handle all yank operations in one place.
    async fn yank(&self, kind: CatalogKind, reference: &CatalogRef) -> anyhow::Result<()>;

    //
    // Providers
    //

    /// Promote an existing provider to a new track/version state.
    async fn promote_provider(
        &self,
        reference: &CatalogRef,
        track: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        self.promote(CatalogKind::Provider, reference, track, version)
            .await
    }

    /// Mark an existing provider as deprecated with an explicit reason.
    async fn deprecate_provider(&self, reference: &CatalogRef, reason: &str) -> anyhow::Result<()> {
        self.deprecate(CatalogKind::Provider, reference, reason)
            .await
    }

    /// Yank (disable) an existing provider from availability.
    async fn yank_provider(&self, reference: &CatalogRef) -> anyhow::Result<()> {
        self.yank(CatalogKind::Provider, reference).await
    }

    //
    // Modules
    //

    /// Promote an existing module to a new track/version state.
    async fn promote_module(
        &self,
        reference: &CatalogRef,
        track: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        self.promote(CatalogKind::Module, reference, track, version)
            .await
    }

    /// Mark an existing module as deprecated with an explicit reason.
    async fn deprecate_module(&self, reference: &CatalogRef, reason: &str) -> anyhow::Result<()> {
        self.deprecate(CatalogKind::Module, reference, reason).await
    }

    /// Yank (disable) an existing module from availability.
    async fn yank_module(&self, reference: &CatalogRef) -> anyhow::Result<()> {
        self.yank(CatalogKind::Module, reference).await
    }

    //
    // Stacks
    //

    /// Promote an existing stack to a new track/version state.
    async fn promote_stack(
        &self,
        reference: &CatalogRef,
        track: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        self.promote(CatalogKind::Stack, reference, track, version)
            .await
    }

    /// Mark an existing stack as deprecated with an explicit reason.
    async fn deprecate_stack(&self, reference: &CatalogRef, reason: &str) -> anyhow::Result<()> {
        self.deprecate(CatalogKind::Stack, reference, reason).await
    }

    /// Yank (disable) an existing stack from availability.
    async fn yank_stack(&self, reference: &CatalogRef) -> anyhow::Result<()> {
        self.yank(CatalogKind::Stack, reference).await
    }
}
