use catalog_client::{materialize_content, CatalogClient};
use catalog_client_aws_http::AwsHttpCatalog;
use catalog_http::{build_router, AppState, DefaultCatalogHttpErrorMap, NoopIdentity};
use catalog_mem::MemCatalog;
use catalog_trait::read::ContentSource;
use catalog_trait::types::TerraformInterface;
use catalog_trait::types::{Metadata, VersionSelector};
use catalog_trait::{CatalogPopulate, CatalogRead, ModuleManifest};

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

#[tokio::test]
async fn http_client_downloads_module_bytes_via_catalog_client() {
    let mem = MemCatalog::default();
    mem.add_module(
        &meta("foo", "main", "1.0.0"),
        &ModuleManifest::default(),
        &TerraformInterface::default(),
        b"artifact-bytes",
    )
    .await
    .expect("add module");

    let state = AppState {
        catalog: mem,
        error_map: DefaultCatalogHttpErrorMap,
        require_management_auth: false,
    };
    let app = build_router::<MemCatalog, DefaultCatalogHttpErrorMap, NoopIdentity>(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });

    let base = format!("http://{}", addr);
    let http_cat = AwsHttpCatalog::new(&base);
    let client = CatalogClient::new(http_cat.clone());

    let module = CatalogRead::get_module(
        &client,
        "foo",
        "main",
        VersionSelector::Exact("1.0.0".into()),
    )
    .await
    .expect("get_module")
    .expect("module exists");

    let bytes = materialize_content(
        CatalogRead::download_module(&client, &module.reference)
            .await
            .expect("download module"),
    )
    .await
    .expect("materialize");
    assert_eq!(bytes, b"artifact-bytes");

    let src = CatalogRead::download_module(&http_cat, &module.reference)
        .await
        .expect("download trait");
    assert!(matches!(src, ContentSource::Bytes(_)));
}
