use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::types::VersionSelector;

// --- Types used only by CatalogAvailability ---

/// Per-region availability status for a catalog entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegionStatus {
    Present,
    Missing,
}

/// Availability report: region → status mapping.
/// Format: `<region>: <present|missing>`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AvailabilityReport {
    pub regions: Vec<(String, RegionStatus)>,
}

/// Request to sync a provider to one or more regions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProviderRequest {
    pub name: String,
    pub track: String,
    pub version: VersionSelector,
    /// Regions to sync to. If empty, implementation may interpret as "all configured regions".
    pub regions: Vec<String>,
}

/// Request to sync a module to one or more regions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncModuleRequest {
    pub name: String,
    pub track: String,
    pub version: VersionSelector,
    pub regions: Vec<String>,
}

/// Request to sync a stack to one or more regions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStackRequest {
    pub name: String,
    pub track: String,
    pub version: VersionSelector,
    pub regions: Vec<String>,
}

/// Outcome of a single region sync operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncEntryStatus {
    Success,
    Retriable,
    Fatal,
}

/// Per-region sync plan/result entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEntry {
    pub source: String,
    pub target: String,
    pub status: SyncEntryStatus,
    pub error: Option<String>,
}

/// Full sync response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub before: AvailabilityReport,
    pub after: AvailabilityReport,
    pub sync: Vec<SyncEntry>,
}

// --- Trait ---

/// Trait for replication availability and sync across regions.
///
/// A separate trait from `Catalog`; implementations can support availability/sync
/// independently of full catalog read/write.
#[async_trait]
pub trait CatalogAvailability: Send + Sync {
    //
    // Configured regions
    //

    /// Return the list of configured regions (regions that can be queried or synced).
    async fn configured_regions(&self) -> anyhow::Result<Vec<String>>;

    //
    // Availability queries (provider, module, stack)
    //

    /// Query availability of a provider across configured regions.
    /// Follows the same style as `CatalogRead::get_provider` (name, track, version).
    async fn availability_provider(
        &self,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<AvailabilityReport>;

    /// Query availability of a module across configured regions.
    async fn availability_module(
        &self,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<AvailabilityReport>;

    /// Query availability of a stack across configured regions.
    async fn availability_stack(
        &self,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<AvailabilityReport>;

    //
    // Sync requests (provider, module, stack)
    //

    /// Request sync of a provider to the specified regions.
    async fn sync_provider(&self, request: &SyncProviderRequest) -> anyhow::Result<SyncResult>;

    /// Request sync of a module to the specified regions.
    async fn sync_module(&self, request: &SyncModuleRequest) -> anyhow::Result<SyncResult>;

    /// Request sync of a stack to the specified regions.
    async fn sync_stack(&self, request: &SyncStackRequest) -> anyhow::Result<SyncResult>;
}
