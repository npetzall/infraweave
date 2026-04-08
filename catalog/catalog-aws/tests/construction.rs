//! Unit tests for AwsCatalog construction (Phase 1 exit criteria).
//!
//! Verifies catalog can be constructed in unit and integration contexts.

use catalog_aws::{AwsCatalog, Config};
use catalog_trait::types::VersionSelector;
use catalog_trait::CatalogAvailability;
use catalog_trait::CatalogRead;

#[test]
fn config_for_test_has_local_mode() {
    let config = Config::for_test();
    assert!(config.local_mode);
    assert_eq!(config.region, "us-west-2");
    assert!(config.dynamodb_endpoint.is_some());
    assert!(config.s3_endpoint.is_some());
    assert_ne!(config.provider_mirror_bucket, config.providers_bucket);
}

#[test]
fn config_table_for_kind() {
    let config = Config::for_test();
    assert_eq!(
        config.table_for_kind(catalog_trait::types::CatalogKind::Module),
        "modules"
    );
    assert_eq!(
        config.table_for_kind(catalog_trait::types::CatalogKind::Provider),
        "providers"
    );
    assert_eq!(
        config.table_for_kind(catalog_trait::types::CatalogKind::Stack),
        "stacks"
    );
}

#[tokio::test]
async fn aws_catalog_constructible_from_test_config() {
    let catalog = AwsCatalog::for_test()
        .await
        .expect("construct from test config");
    assert!(catalog.clients().config().local_mode);
}

#[tokio::test]
async fn catalog_availability_configured_regions_matches_client_region() {
    let catalog = AwsCatalog::for_test().await.expect("construct");
    let regions = catalog
        .configured_regions()
        .await
        .expect("configured_regions");
    assert_eq!(regions, vec![catalog.clients().config().region.clone()]);
}

/// Requires local DynamoDB (e.g. DynamoDB Local) to be running.
#[tokio::test]
#[ignore = "requires local DynamoDB; run with --ignored when infrastructure is available"]
async fn catalog_availability_module_missing_reports_missing() {
    let catalog = AwsCatalog::for_test().await.expect("construct");
    let report = catalog
        .availability_module("nonexistent-module-xyz", "default", VersionSelector::Latest)
        .await
        .expect("availability_module");
    assert_eq!(report.regions.len(), 1);
    assert_eq!(report.regions[0].0, catalog.clients().config().region);
    assert_eq!(
        report.regions[0].1,
        catalog_trait::availability::RegionStatus::Missing
    );
}

/// Requires local DynamoDB (e.g. DynamoDB Local) to be running.
#[tokio::test]
#[ignore = "requires local DynamoDB; run with --ignored when infrastructure is available"]
async fn aws_catalog_list_returns_empty_page() {
    let catalog = AwsCatalog::for_test().await.expect("construct");
    let page: catalog_trait::read::Page<catalog_trait::read::CatalogEntry> = catalog
        .list(
            catalog_trait::types::CatalogKind::Module,
            &catalog_trait::read::Query::default(),
        )
        .await
        .expect("list should not fail");
    assert!(page.items.is_empty());
    assert!(page.next.is_none());
}
