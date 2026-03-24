//! In-memory [`Catalog`](catalog_trait::Catalog) implementation for **tests, local demos, and offline
//! development**—no AWS, no network, and no persistence across process restarts.
//!
//! ## Concurrency
//!
//! The catalog state is protected by [`std::sync::RwLock`] around in-memory maps. Many concurrent
//! readers can proceed together; writers take an exclusive lock. This is appropriate for tests and
//! single-process demos, not for high-contention production serving.
//!
//! [`CatalogPopulate`](catalog_trait::CatalogPopulate), [`CatalogRead`](catalog_trait::CatalogRead), and
//! [`CatalogManagement`](catalog_trait::CatalogManagement) are implemented (including
//! [`Query::projection`](catalog_trait::read::Query::projection) and list pagination cursors on
//! [`list`](catalog_trait::CatalogRead::list)). Together they satisfy [`Catalog`](catalog_trait::Catalog).

mod management;
mod populate;
mod read;
mod store;

use std::sync::Arc;

/// In-memory catalog. Intended for tests and local demos; not a production persistence backend.
#[derive(Clone, Debug)]
pub struct MemCatalog {
    pub(crate) store: Arc<store::Store>,
}

impl Default for MemCatalog {
    fn default() -> Self {
        Self {
            store: Arc::new(store::Store::default()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine as _;
    use catalog_trait::read::{CatalogEntry, ContentSource, Module, ProjectionFields, Query};
    use catalog_trait::types::{CatalogKind, Metadata, TerraformInterface, VersionSelector};
    use catalog_trait::{
        Catalog, CatalogManagement, CatalogPopulate, CatalogRead, ModuleManifest, ModuleStackData,
        StackManifest, StackMetadata, StackSpec,
    };

    fn meta(name: &str, track: &str, version: &str) -> Metadata {
        Metadata {
            name: name.into(),
            kind: "".into(),
            track: track.into(),
            version: version.into(),
            timestamp: "".into(),
            description: "".into(),
            reference: "".into(),
            cpu: "".into(),
            memory: "".into(),
            deprecated: false,
            deprecated_message: None,
        }
    }

    #[test]
    fn mem_catalog_inserts_provider_and_reads_back() {
        let catalog = MemCatalog::default();
        let manifest: catalog_trait::ProviderManifest = serde_json::from_str(
            r#"{
                "metadata": { "name": "p" },
                "apiVersion": "v1",
                "kind": "Provider",
                "spec": {
                    "provider": "aws",
                    "description": "",
                    "reference": ""
                }
            }"#,
        )
        .expect("valid provider JSON");

        let reference = catalog.store.insert_provider(
            meta("aws", "default", "1.0.0"),
            manifest,
            TerraformInterface::default(),
            b"zip-bytes".to_vec(),
        );

        let entry = catalog
            .store
            .get_entry(&reference.id)
            .expect("stored entry");
        assert_eq!(entry.kind, CatalogKind::Provider);
        assert_eq!(entry.metadata.name, "aws");
        assert_eq!(entry.content, b"zip-bytes");
        assert!(matches!(
            entry.kind_payload,
            crate::store::KindPayload::Provider(_)
        ));
    }

    #[test]
    fn mem_catalog_inserts_module_and_round_trips_logical_index() {
        let catalog = MemCatalog::default();
        let reference = catalog.store.insert_module(
            meta("MyMod", "default", "0.1.0"),
            ModuleManifest::default(),
            TerraformInterface::default(),
            vec![],
        );

        let ids = catalog
            .store
            .logical_version_ids(CatalogKind::Module, "MyMod", "default");
        assert_eq!(ids, vec![reference.id.clone()]);

        let entry = catalog.store.get_entry(&reference.id).expect("entry");
        assert_eq!(entry.metadata.version, "0.1.0");
    }

    #[test]
    fn mem_catalog_stack_and_attachment() {
        let catalog = MemCatalog::default();
        let stack_manifest = StackManifest {
            metadata: StackMetadata {
                name: "stack-a".into(),
            },
            api_version: "v1".into(),
            kind: "Stack".into(),
            spec: StackSpec {
                stack_name: "StackA".into(),
                version: Some("1.0.0".into()),
                description: "".into(),
                reference: "".into(),
                examples: None,
                cpu: None,
                memory: None,
                locals: None,
                dependencies: None,
                stack_variable_definitions: None,
            },
        };

        let reference = catalog.store.insert_stack(
            meta("stack-a", "default", "1.0.0"),
            stack_manifest,
            TerraformInterface::default(),
            None,
            vec![],
        );

        catalog
            .store
            .insert_attachment(&reference.id, "sbom.json", b"{\"ok\":true}".to_vec())
            .expect("attachment");

        let entry = catalog.store.get_entry(&reference.id).expect("entry");
        assert_eq!(
            entry.attachments.get("sbom.json").map(Vec::as_slice),
            Some(b"{\"ok\":true}".as_slice())
        );
    }

    /// Populate via trait, then read back primary bytes from the store.
    #[tokio::test]
    async fn populate_provider_then_content_matches() {
        let catalog = MemCatalog::default();
        let manifest: catalog_trait::ProviderManifest = serde_json::from_str(
            r#"{
                "metadata": { "name": "p" },
                "apiVersion": "v1",
                "kind": "Provider",
                "spec": {
                    "provider": "aws",
                    "description": "",
                    "reference": ""
                }
            }"#,
        )
        .expect("valid provider JSON");
        let zip = b"zip-bytes";
        let reference = catalog
            .add_provider(
                &meta("aws", "default", "1.0.0"),
                &manifest,
                &TerraformInterface::default(),
                zip,
            )
            .await
            .expect("add_provider");

        let entry = catalog
            .store
            .get_entry(&reference.id)
            .expect("stored entry");
        assert_eq!(entry.content.as_slice(), zip);
    }

    #[tokio::test]
    async fn populate_module_stack_attachment_round_trip() {
        let catalog = MemCatalog::default();
        let module_ref = catalog
            .add_module(
                &meta("M", "default", "0.2.0"),
                &ModuleManifest::default(),
                &TerraformInterface::default(),
                b"mod-bin",
            )
            .await
            .expect("add_module");
        let entry = catalog
            .store
            .get_entry(&module_ref.id)
            .expect("module entry");
        assert_eq!(entry.content.as_slice(), b"mod-bin");

        let stack_manifest = StackManifest {
            metadata: StackMetadata { name: "s".into() },
            api_version: "v1".into(),
            kind: "Stack".into(),
            spec: StackSpec {
                stack_name: "S".into(),
                version: Some("1.0.0".into()),
                description: "".into(),
                reference: "".into(),
                examples: None,
                cpu: None,
                memory: None,
                locals: None,
                dependencies: None,
                stack_variable_definitions: None,
            },
        };
        let stack_ref = catalog
            .add_stack(
                &meta("s", "default", "1.0.0"),
                &stack_manifest,
                &TerraformInterface::default(),
                None,
                b"stack-bin",
            )
            .await
            .expect("add_stack");
        assert_eq!(
            catalog
                .store
                .get_entry(&stack_ref.id)
                .expect("stack entry")
                .content
                .as_slice(),
            b"stack-bin"
        );

        catalog
            .add_attachment(&stack_ref, "a.txt", b"att")
            .await
            .expect("add_attachment");
        let again = catalog.store.get_entry(&stack_ref.id).expect("stack entry");
        assert_eq!(
            again.attachments.get("a.txt").map(Vec::as_slice),
            Some(b"att".as_slice())
        );
    }

    #[tokio::test]
    async fn add_attachment_unknown_id_errors() {
        let catalog = MemCatalog::default();
        let err = catalog
            .add_attachment(
                &catalog_trait::types::CatalogRef {
                    id: "not-a-uuid".into(),
                },
                "x",
                b"y",
            )
            .await
            .expect_err("unknown id");
        assert!(err.to_string().contains("unknown catalog id"));
    }

    #[tokio::test]
    async fn read_get_latest_uses_highest_semver() {
        let catalog = MemCatalog::default();
        let manifest: catalog_trait::ProviderManifest = serde_json::from_str(
            r#"{
                "metadata": { "name": "p" },
                "apiVersion": "v1",
                "kind": "Provider",
                "spec": { "provider": "aws", "description": "", "reference": "" }
            }"#,
        )
        .expect("valid provider JSON");

        for ver in ["1.0.0", "2.0.0", "1.5.0"] {
            catalog
                .add_provider(
                    &meta("aws", "default", ver),
                    &manifest,
                    &TerraformInterface::default(),
                    ver.as_bytes(),
                )
                .await
                .expect("add_provider");
        }

        let got = catalog
            .get_provider("aws", "default", VersionSelector::Latest)
            .await
            .expect("get")
            .expect("some");
        let bytes = match catalog.download_provider(&got.reference).await.expect("dl") {
            ContentSource::Bytes(b) => b,
            other => panic!("expected bytes, got {other:?}"),
        };
        assert_eq!(bytes.as_slice(), b"2.0.0");
    }

    #[tokio::test]
    async fn read_list_filters_and_default_helpers() {
        let catalog = MemCatalog::default();
        catalog
            .add_module(
                &meta("A", "default", "1.0.0"),
                &ModuleManifest::default(),
                &TerraformInterface::default(),
                b"a1",
            )
            .await
            .unwrap();
        catalog
            .add_module(
                &meta("B", "beta", "0.1.0"),
                &ModuleManifest::default(),
                &TerraformInterface::default(),
                b"b1",
            )
            .await
            .unwrap();

        let only_a = catalog
            .list_modules(&Query {
                name: Some("A".into()),
                ..Default::default()
            })
            .await
            .expect("list");
        assert_eq!(only_a.items.len(), 1);
        assert_eq!(only_a.next, None);

        let page = catalog
            .list(CatalogKind::Module, &Query::default())
            .await
            .expect("list all");
        assert_eq!(page.items.len(), 2);
    }

    #[tokio::test]
    async fn list_pagination_pages_cover_all_rows_once() {
        use std::collections::HashSet;

        let catalog = MemCatalog::default();
        for i in 0..5 {
            catalog
                .add_module(
                    &meta(&format!("M{i}"), "default", "1.0.0"),
                    &ModuleManifest::default(),
                    &TerraformInterface::default(),
                    b"x",
                )
                .await
                .unwrap();
        }

        let mut seen = HashSet::new();
        let mut next: Option<String> = None;
        loop {
            let page = catalog
                .list(
                    CatalogKind::Module,
                    &Query {
                        limit: Some(2),
                        next: next.clone(),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            for item in &page.items {
                let id = match item {
                    CatalogEntry::Module(m) => m.reference.id.clone(),
                    _ => panic!("expected module"),
                };
                assert!(seen.insert(id), "duplicate row across pages");
            }
            next = page.next;
            if next.is_none() {
                break;
            }
        }
        assert_eq!(seen.len(), 5);
    }

    #[tokio::test]
    async fn list_pagination_invalid_next_token_errors() {
        let catalog = MemCatalog::default();
        let err = catalog
            .list(
                CatalogKind::Module,
                &Query {
                    next: Some("%%%not-valid-base64%%%".into()),
                    ..Default::default()
                },
            )
            .await
            .expect_err("bad token");
        assert!(err
            .to_string()
            .contains("invalid catalog-mem list continuation token"));
    }

    #[tokio::test]
    async fn list_pagination_offset_past_end_errors() {
        let catalog = MemCatalog::default();
        catalog
            .add_module(
                &meta("only", "default", "1.0.0"),
                &ModuleManifest::default(),
                &TerraformInterface::default(),
                b"x",
            )
            .await
            .unwrap();

        let token = B64.encode(serde_json::to_vec(&serde_json::json!({ "offset": 5 })).unwrap());
        let err = catalog
            .list(
                CatalogKind::Module,
                &Query {
                    next: Some(token),
                    ..Default::default()
                },
            )
            .await
            .expect_err("stale offset");
        assert!(err
            .to_string()
            .contains("invalid catalog-mem list continuation token"));
    }

    #[tokio::test]
    async fn read_attachments_sorted_and_download() {
        let catalog = MemCatalog::default();
        let m = catalog
            .add_module(
                &meta("M", "default", "1.0.0"),
                &ModuleManifest::default(),
                &TerraformInterface::default(),
                b"x",
            )
            .await
            .unwrap();
        catalog.add_attachment(&m, "z.txt", b"z").await.unwrap();
        catalog.add_attachment(&m, "a.txt", b"a").await.unwrap();

        let names = catalog.list_attachments(&m).await.expect("names");
        assert_eq!(names, vec!["a.txt", "z.txt"]);

        let ContentSource::Bytes(got) =
            catalog.download_attachment(&m, "z.txt").await.expect("att")
        else {
            panic!("expected bytes");
        };
        assert_eq!(got.as_slice(), b"z");
    }

    #[tokio::test]
    async fn download_module_rejects_provider_id() {
        let catalog = MemCatalog::default();
        let manifest: catalog_trait::ProviderManifest = serde_json::from_str(
            r#"{
                "metadata": { "name": "p" },
                "apiVersion": "v1",
                "kind": "Provider",
                "spec": { "provider": "aws", "description": "", "reference": "" }
            }"#,
        )
        .unwrap();
        let pref = catalog
            .add_provider(
                &meta("aws", "default", "1.0.0"),
                &manifest,
                &TerraformInterface::default(),
                b"z",
            )
            .await
            .unwrap();
        let err = catalog
            .download_module(&pref)
            .await
            .expect_err("wrong kind");
        assert!(err.to_string().contains("not a module"));
    }

    #[tokio::test]
    async fn list_projection_provider_full_and_metadata_only() {
        let catalog = MemCatalog::default();
        let manifest: catalog_trait::ProviderManifest = serde_json::from_str(
            r#"{
                "metadata": { "name": "p" },
                "apiVersion": "v1",
                "kind": "Provider",
                "spec": { "provider": "aws", "description": "", "reference": "" }
            }"#,
        )
        .unwrap();
        catalog
            .add_provider(
                &meta("aws", "default", "1.0.0"),
                &manifest,
                &TerraformInterface::default(),
                b"z",
            )
            .await
            .unwrap();

        let full = catalog
            .list(
                CatalogKind::Provider,
                &Query {
                    projection: None,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let CatalogEntry::Provider(p_full) = &full.items[0] else {
            panic!("expected provider");
        };
        assert!(p_full.metadata.is_some());
        assert!(p_full.manifest.is_some());
        assert!(p_full.terraform.is_some());

        let masked = catalog
            .list(
                CatalogKind::Provider,
                &Query {
                    projection: Some(ProjectionFields::METADATA),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let CatalogEntry::Provider(p_meta) = &masked.items[0] else {
            panic!("expected provider");
        };
        assert!(p_meta.metadata.is_some());
        assert!(p_meta.manifest.is_none());
        assert!(p_meta.terraform.is_none());
    }

    #[tokio::test]
    async fn list_projection_module_full_and_metadata_only() {
        let catalog = MemCatalog::default();
        catalog
            .add_module(
                &meta("M", "default", "1.0.0"),
                &ModuleManifest::default(),
                &TerraformInterface::default(),
                b"x",
            )
            .await
            .unwrap();

        let full = catalog
            .list(
                CatalogKind::Module,
                &Query {
                    projection: None,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let CatalogEntry::Module(m_full) = &full.items[0] else {
            panic!("expected module");
        };
        assert!(m_full.metadata.is_some());
        assert!(m_full.manifest.is_some());
        assert!(m_full.terraform.is_some());

        let masked = catalog
            .list(
                CatalogKind::Module,
                &Query {
                    projection: Some(ProjectionFields::METADATA),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let CatalogEntry::Module(m_meta) = &masked.items[0] else {
            panic!("expected module");
        };
        assert!(m_meta.metadata.is_some());
        assert!(m_meta.manifest.is_none());
        assert!(m_meta.terraform.is_none());
    }

    #[tokio::test]
    async fn list_projection_stack_full_and_without_stack_data() {
        let catalog = MemCatalog::default();
        let stack_manifest = StackManifest {
            metadata: StackMetadata {
                name: "stack-a".into(),
            },
            api_version: "v1".into(),
            kind: "Stack".into(),
            spec: StackSpec {
                stack_name: "StackA".into(),
                version: Some("1.0.0".into()),
                description: "".into(),
                reference: "".into(),
                examples: None,
                cpu: None,
                memory: None,
                locals: None,
                dependencies: None,
                stack_variable_definitions: None,
            },
        };
        catalog
            .add_stack(
                &meta("stack-a", "default", "1.0.0"),
                &stack_manifest,
                &TerraformInterface::default(),
                Some(ModuleStackData { modules: vec![] }),
                b"s",
            )
            .await
            .unwrap();

        let full = catalog
            .list(
                CatalogKind::Stack,
                &Query {
                    projection: None,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let CatalogEntry::Stack(s_full) = &full.items[0] else {
            panic!("expected stack");
        };
        assert!(s_full.metadata.is_some());
        assert!(s_full.manifest.is_some());
        assert!(s_full.terraform.is_some());
        assert_eq!(s_full.stack_data.as_ref().map(|d| d.modules.len()), Some(0));

        let without_data = catalog
            .list(
                CatalogKind::Stack,
                &Query {
                    projection: Some(
                        ProjectionFields::METADATA
                            | ProjectionFields::MANIFEST
                            | ProjectionFields::TERRAFORM,
                    ),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let CatalogEntry::Stack(s_trim) = &without_data.items[0] else {
            panic!("expected stack");
        };
        assert!(s_trim.metadata.is_some());
        assert!(s_trim.manifest.is_some());
        assert!(s_trim.stack_data.is_none());
    }

    #[tokio::test]
    async fn management_promote_then_get_by_new_track_and_version() {
        let catalog = MemCatalog::default();
        let module_ref = catalog
            .add_module(
                &meta("M", "default", "1.0.0"),
                &ModuleManifest::default(),
                &catalog_trait::types::TerraformInterface::default(),
                b"m1",
            )
            .await
            .unwrap();

        catalog
            .promote_module(&module_ref, "beta", None)
            .await
            .expect("promote");

        let m = catalog
            .get_module("M", "beta", VersionSelector::Exact("1.0.0".into()))
            .await
            .unwrap()
            .expect("entry");
        assert_eq!(m.reference.id, module_ref.id);
        assert_eq!(m.metadata.as_ref().expect("metadata").track, "beta");

        let _: &dyn Catalog = &catalog;
    }

    #[tokio::test]
    async fn management_deprecate_sets_flags_and_still_lists() {
        let catalog = MemCatalog::default();
        let module_ref = catalog
            .add_module(
                &meta("M", "default", "1.0.0"),
                &ModuleManifest::default(),
                &catalog_trait::types::TerraformInterface::default(),
                b"x",
            )
            .await
            .unwrap();

        catalog
            .deprecate_module(&module_ref, "use v2")
            .await
            .unwrap();

        let page = catalog.list_modules(&Query::default()).await.unwrap();
        assert_eq!(page.items.len(), 1);
        let m: &Module = &page.items[0];
        let md = m.metadata.as_ref().unwrap();
        assert!(md.deprecated);
        assert_eq!(md.deprecated_message.as_deref(), Some("use v2"));

        let latest = catalog
            .get_module("M", "default", VersionSelector::Latest)
            .await
            .unwrap()
            .expect("latest still resolves");
        let _: &Module = &latest;
    }

    #[tokio::test]
    async fn management_yank_hides_from_list_get_latest_and_blocks_download() {
        let catalog = MemCatalog::default();
        let module_ref = catalog
            .add_module(
                &meta("M", "default", "1.0.0"),
                &ModuleManifest::default(),
                &catalog_trait::types::TerraformInterface::default(),
                b"bytes",
            )
            .await
            .unwrap();

        catalog.yank_module(&module_ref).await.unwrap();

        let page = catalog.list_modules(&Query::default()).await.unwrap();
        assert!(page.items.is_empty());

        assert!(catalog
            .get_module("M", "default", VersionSelector::Latest)
            .await
            .unwrap()
            .is_none());

        let err = catalog.download_module(&module_ref).await.unwrap_err();
        assert!(err.to_string().contains("yanked"));
    }
}
