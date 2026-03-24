//! Ensures catalog-owned types round-trip through JSON.

use catalog_trait::availability::{
    AvailabilityReport, RegionStatus, SyncEntry, SyncEntryStatus, SyncModuleRequest,
    SyncProviderRequest, SyncResult, SyncStackRequest,
};
use catalog_trait::read::{
    CatalogEntry, ContentSource, Module, Page, ProjectionFields, Provider, Query, Stack,
};
use catalog_trait::types::{
    CatalogKind, CatalogRef, Metadata, TerraformInterface, VersionSelector,
};
use std::collections::HashMap;
use std::path::PathBuf;

fn roundtrip_json<T>(v: &T)
where
    T: serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug,
{
    let s = serde_json::to_string(v).expect("serialize");
    let back: T = serde_json::from_str(&s).expect("deserialize");
    let s2 = serde_json::to_string(&back).expect("re-serialize");
    assert_eq!(s, s2, "roundtrip mismatch");
}

#[test]
fn types_roundtrip() {
    roundtrip_json(&CatalogRef { id: "ref-1".into() });
    roundtrip_json(&Metadata {
        name: "n".into(),
        kind: "k".into(),
        track: "t".into(),
        version: "1".into(),
        timestamp: "ts".into(),
        description: "d".into(),
        reference: "r".into(),
        cpu: "c".into(),
        memory: "m".into(),
        deprecated: false,
        deprecated_message: Some("msg".into()),
    });
    roundtrip_json(&TerraformInterface::default());
    roundtrip_json(&CatalogKind::Module);
    roundtrip_json(&VersionSelector::Latest);
    roundtrip_json(&VersionSelector::Exact("1.0.0".into()));
}

#[test]
fn availability_roundtrip() {
    roundtrip_json(&RegionStatus::Present);
    roundtrip_json(&AvailabilityReport {
        regions: vec![
            ("eu-west-1".into(), RegionStatus::Present),
            ("us-east-1".into(), RegionStatus::Missing),
        ],
    });
    roundtrip_json(&SyncProviderRequest {
        name: "p".into(),
        track: "stable".into(),
        version: VersionSelector::Latest,
        regions: vec!["a".into()],
    });
    roundtrip_json(&SyncModuleRequest {
        name: "m".into(),
        track: "stable".into(),
        version: VersionSelector::Exact("1.0.0".into()),
        regions: vec![],
    });
    roundtrip_json(&SyncStackRequest {
        name: "s".into(),
        track: "stable".into(),
        version: VersionSelector::Latest,
        regions: vec!["b".into()],
    });
    roundtrip_json(&SyncEntryStatus::Retriable);
    roundtrip_json(&SyncEntry {
        source: "src".into(),
        target: "dst".into(),
        status: SyncEntryStatus::Success,
        error: None,
    });
    roundtrip_json(&SyncResult {
        before: AvailabilityReport::default(),
        after: AvailabilityReport {
            regions: vec![("r".into(), RegionStatus::Missing)],
        },
        sync: vec![],
    });
}

#[test]
fn read_roundtrip() {
    roundtrip_json(&ProjectionFields::ALL);
    roundtrip_json(&ContentSource::Url("https://x".into()));
    roundtrip_json(&ContentSource::Path(std::path::PathBuf::from("/tmp/x")));
    roundtrip_json(&ContentSource::Bytes(vec![1, 2, 3]));

    let cref = CatalogRef { id: "id".into() };
    roundtrip_json(&Provider::new(cref.clone()));
    roundtrip_json(&Module::new(cref.clone()));

    let mut mirror = HashMap::new();
    mirror.insert(
        PathBuf::from("registry.terraform.io/hashicorp/aws"),
        ContentSource::Url("https://mirror.example/aws.zip".into()),
    );
    roundtrip_json(&Module {
        reference: cref.clone(),
        metadata: None,
        manifest: None,
        terraform: None,
        provider_mirror: Some(mirror),
    });

    roundtrip_json(&Stack::new(cref));

    roundtrip_json(&Page::<Provider> {
        items: vec![],
        next: Some("token".into()),
    });

    roundtrip_json(&Query {
        name: Some("n".into()),
        track: Some("t".into()),
        limit: Some(10),
        next: None,
        projection: Some(ProjectionFields::METADATA | ProjectionFields::MANIFEST),
    });

    let p = Provider::new(CatalogRef { id: "p".into() });
    roundtrip_json(&CatalogEntry::Provider(p));
}
