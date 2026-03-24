//! Regional availability and sync (`CatalogAvailability`).
//!
//! This deployment uses one DynamoDB/S3 region per [`crate::client::AwsClients`]. Configured
//! regions are the client’s primary region; cross-region replication is not performed here—sync
//! entries report `Fatal` when the artifact is missing or the target region is not this client.

use std::collections::HashMap;

use async_trait::async_trait;
use aws_sdk_dynamodb::types::AttributeValue;
use catalog_trait::availability::{
    AvailabilityReport, RegionStatus, SyncEntry, SyncEntryStatus, SyncModuleRequest,
    SyncProviderRequest, SyncResult, SyncStackRequest,
};
use catalog_trait::types::{CatalogKind, VersionSelector};
use catalog_trait::CatalogAvailability;

use crate::client::AwsClients;
use crate::config::Config;
use crate::ops;
use crate::read;

/// Whether the DynamoDB row is considered available for use (present and not yanked).
fn region_status_for_item(
    kind: CatalogKind,
    item: Option<HashMap<String, AttributeValue>>,
) -> Result<RegionStatus, anyhow::Error> {
    let Some(item) = item else {
        return Ok(RegionStatus::Missing);
    };
    match kind {
        CatalogKind::Provider => {
            let r = read::item_to_provider(&item).map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok(if r.yanked {
                RegionStatus::Missing
            } else {
                RegionStatus::Present
            })
        }
        CatalogKind::Module | CatalogKind::Stack => {
            let r = read::item_to_module(&item).map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok(if r.yanked {
                RegionStatus::Missing
            } else {
                RegionStatus::Present
            })
        }
    }
}

async fn availability_for_kind(
    clients: &AwsClients,
    config: &Config,
    kind: CatalogKind,
    name: &str,
    track: &str,
    version: VersionSelector,
) -> anyhow::Result<AvailabilityReport> {
    let region = config.region.clone();
    let item = ops::execute_get(clients, config, kind, name, track, &version).await?;
    let status = region_status_for_item(kind, item)?;
    Ok(AvailabilityReport {
        regions: vec![(region, status)],
    })
}

fn sync_target_regions(request_regions: &[String], configured: &[String]) -> Vec<String> {
    if request_regions.is_empty() {
        return configured.to_vec();
    }
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for r in request_regions {
        if seen.insert(r.as_str()) {
            out.push(r.clone());
        }
    }
    out
}

async fn sync_kind(
    clients: &AwsClients,
    config: &Config,
    kind: CatalogKind,
    name: &str,
    track: &str,
    version: VersionSelector,
    target_regions: &[String],
) -> anyhow::Result<SyncResult> {
    let source = config.region.clone();
    let configured: Vec<String> = vec![source.clone()];

    let before = availability_for_kind(clients, config, kind, name, track, version.clone()).await?;

    let targets = sync_target_regions(target_regions, &configured);
    let mut sync_entries = Vec::new();

    for target in targets {
        if target != source {
            sync_entries.push(SyncEntry {
                source: source.clone(),
                target: target.clone(),
                status: SyncEntryStatus::Fatal,
                error: Some(format!(
                    "region {target} is not configured on this catalog client (configured: {source})"
                )),
            });
            continue;
        }

        let status_before = before
            .regions
            .iter()
            .find(|(r, _)| r == &target)
            .map(|(_, s)| *s)
            .unwrap_or(RegionStatus::Missing);

        if status_before == RegionStatus::Present {
            sync_entries.push(SyncEntry {
                source: source.clone(),
                target: target.clone(),
                status: SyncEntryStatus::Success,
                error: None,
            });
        } else {
            sync_entries.push(SyncEntry {
                source: source.clone(),
                target: target.clone(),
                status: SyncEntryStatus::Fatal,
                error: Some(
                    "artifact not present in this region; cross-region replication is not implemented for catalog-aws"
                        .to_string(),
                ),
            });
        }
    }

    let after = availability_for_kind(clients, config, kind, name, track, version).await?;

    Ok(SyncResult {
        before,
        after,
        sync: sync_entries,
    })
}

#[async_trait]
impl CatalogAvailability for crate::AwsCatalog {
    async fn configured_regions(&self) -> anyhow::Result<Vec<String>> {
        Ok(vec![self.clients.config().region.clone()])
    }

    async fn availability_provider(
        &self,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<AvailabilityReport> {
        availability_for_kind(
            &self.clients,
            self.clients.config(),
            CatalogKind::Provider,
            name,
            track,
            version,
        )
        .await
    }

    async fn availability_module(
        &self,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<AvailabilityReport> {
        availability_for_kind(
            &self.clients,
            self.clients.config(),
            CatalogKind::Module,
            name,
            track,
            version,
        )
        .await
    }

    async fn availability_stack(
        &self,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<AvailabilityReport> {
        availability_for_kind(
            &self.clients,
            self.clients.config(),
            CatalogKind::Stack,
            name,
            track,
            version,
        )
        .await
    }

    async fn sync_provider(&self, request: &SyncProviderRequest) -> anyhow::Result<SyncResult> {
        sync_kind(
            &self.clients,
            self.clients.config(),
            CatalogKind::Provider,
            &request.name,
            &request.track,
            request.version.clone(),
            &request.regions,
        )
        .await
    }

    async fn sync_module(&self, request: &SyncModuleRequest) -> anyhow::Result<SyncResult> {
        sync_kind(
            &self.clients,
            self.clients.config(),
            CatalogKind::Module,
            &request.name,
            &request.track,
            request.version.clone(),
            &request.regions,
        )
        .await
    }

    async fn sync_stack(&self, request: &SyncStackRequest) -> anyhow::Result<SyncResult> {
        sync_kind(
            &self.clients,
            self.clients.config(),
            CatalogKind::Stack,
            &request.name,
            &request.track,
            request.version.clone(),
            &request.regions,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_target_regions_empty_uses_configured() {
        let configured = vec!["us-east-1".to_string()];
        assert_eq!(
            sync_target_regions(&[], &configured),
            vec!["us-east-1".to_string()]
        );
    }

    #[test]
    fn sync_target_regions_dedupes_preserves_order() {
        let configured = vec![];
        assert_eq!(
            sync_target_regions(
                &["b".to_string(), "a".to_string(), "b".to_string()],
                &configured
            ),
            vec!["b".to_string(), "a".to_string()]
        );
    }
}
