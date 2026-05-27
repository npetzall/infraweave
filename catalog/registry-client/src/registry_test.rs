use super::*;
use reqwest::StatusCode;

#[test]
fn builtin_registry_hosts_case_insensitive() {
    assert!(Registry::is_builtin_host("REGISTRY.TERRAFORM.IO"));
    assert!(Registry::is_builtin_host("Registry.OpenTofu.Org"));
    assert!(!Registry::is_builtin_host("registry.example.com"));
}

#[test]
fn resolve_service_v1_relative_to_discovery_url() {
    let discovery =
        Url::parse("http://127.0.0.1:9/.well-known/terraform.json").expect("discovery url");
    assert_eq!(
        Registry::resolve_service_v1_base("/api/v1/providers/", &discovery).unwrap(),
        "http://127.0.0.1:9/api/v1/providers/"
    );
}

#[test]
fn resolve_service_v1_relative_without_leading_slash_uses_discovery_path() {
    let discovery =
        Url::parse("http://127.0.0.1:9/discovery/terraform.json").expect("discovery url");
    assert_eq!(
        Registry::resolve_service_v1_base("reg/providers/", &discovery).unwrap(),
        "http://127.0.0.1:9/discovery/reg/providers/"
    );
}

#[test]
fn resolve_service_v1_absolute_passes_through() {
    let discovery = Url::parse("http://ignored.example/.well-known/terraform.json").unwrap();
    assert_eq!(
        Registry::resolve_service_v1_base("http://127.0.0.1:9/custom/providers/", &discovery)
            .unwrap(),
        "http://127.0.0.1:9/custom/providers/"
    );
}

#[tokio::test]
async fn provider_rejects_whitespace_only_host() {
    let reg = Registry::new("  \t  ");
    let http = reqwest::Client::new();
    let err = reg.provider(&http).await.unwrap_err();
    assert!(matches!(err, ProviderRegistryError::EmptyRegistryHost));
}

#[tokio::test]
async fn module_rejects_whitespace_only_host() {
    let reg = Registry::new("  \t  ");
    let http = reqwest::Client::new();
    let err = reg.module(&http).await.unwrap_err();
    assert!(matches!(err, ProviderRegistryError::EmptyRegistryHost));
}

#[tokio::test]
async fn provider_discovery_non_success_status() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let reg = Registry::new(&reg_host);
    let http = reqwest::Client::new();
    let err = reg.provider(&http).await.unwrap_err();
    assert!(matches!(
        err,
        ProviderRegistryError::UnsuccessfulStatus {
            status: StatusCode::SERVICE_UNAVAILABLE,
            ..
        }
    ));
}

#[tokio::test]
async fn provider_discovery_invalid_json() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("not-json {{{", "application/json"))
        .mount(&server)
        .await;

    let reg = Registry::new(&reg_host);
    let http = reqwest::Client::new();
    let err = reg.provider(&http).await.unwrap_err();
    assert!(matches!(err, ProviderRegistryError::Json { .. }));
}

#[tokio::test]
async fn provider_discovery_wrong_content_type() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"providers.v1":"/v1/providers/"}"#)
                .insert_header("Content-Type", "text/plain"),
        )
        .mount(&server)
        .await;

    let reg = Registry::new(&reg_host);
    let http = reqwest::Client::new();
    let err = reg.provider(&http).await.unwrap_err();
    assert!(matches!(
        err,
        ProviderRegistryError::UnexpectedContentType { .. }
    ));
}

#[tokio::test]
async fn provider_discovery_redirect_relative_providers_v1() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");
    let redirect_target = format!("http://127.0.0.1:{port}/discovery/terraform.json");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", redirect_target.as_str()),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/discovery/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "providers.v1": "reg/providers/"
        })))
        .mount(&server)
        .await;

    let reg = Registry::new(&reg_host);
    let http = reqwest::Client::new();
    let pr = reg.provider(&http).await.expect("provider");
    assert_eq!(
        pr.base_url().as_str(),
        format!("http://127.0.0.1:{port}/discovery/reg/providers/")
    );
}

#[tokio::test]
async fn module_discovery_redirect_relative_modules_v1() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");
    let redirect_target = format!("http://127.0.0.1:{port}/discovery/terraform.json");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", redirect_target.as_str()),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/discovery/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "modules.v1": "reg/modules/"
        })))
        .mount(&server)
        .await;

    let reg = Registry::new(&reg_host);
    let http = reqwest::Client::new();
    let mr = reg.module(&http).await.expect("module");
    assert_eq!(
        mr.base_url().as_str(),
        format!("http://127.0.0.1:{port}/discovery/reg/modules/")
    );
}

#[tokio::test]
async fn provider_discovery_only_providers_v1() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "providers.v1": format!("http://127.0.0.1:{port}/v1/providers/")
        })))
        .mount(&server)
        .await;

    let reg = Registry::new(&reg_host);
    let http = reqwest::Client::new();
    reg.provider(&http)
        .await
        .expect("only providers.v1 is enough");
}

#[tokio::test]
async fn module_discovery_only_modules_v1() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "modules.v1": format!("http://127.0.0.1:{port}/v1/modules/")
        })))
        .mount(&server)
        .await;

    let reg = Registry::new(&reg_host);
    let http = reqwest::Client::new();
    reg.module(&http).await.expect("only modules.v1 is enough");
}

#[tokio::test]
async fn discovery_missing_providers_v1() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "modules.v1": format!("http://127.0.0.1:{port}/v1/modules/")
        })))
        .mount(&server)
        .await;

    let reg = Registry::new(&reg_host);
    let http = reqwest::Client::new();
    let err = reg.provider(&http).await.unwrap_err();
    assert!(matches!(
        err,
        ProviderRegistryError::MissingProvidersV1 { .. }
    ));
}

#[tokio::test]
async fn discovery_missing_modules_v1() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "providers.v1": format!("http://127.0.0.1:{port}/v1/providers/")
        })))
        .mount(&server)
        .await;

    let reg = Registry::new(&reg_host);
    let http = reqwest::Client::new();
    let err = reg.module(&http).await.unwrap_err();
    assert!(matches!(
        err,
        ProviderRegistryError::MissingModulesV1 { .. }
    ));
}

#[tokio::test]
async fn discovery_both_services_single_fetch() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;
    let port = server.address().port();
    let reg_host = format!("127.0.0.1:{port}");

    Mock::given(method("GET"))
        .and(path("/.well-known/terraform.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "providers.v1": format!("http://127.0.0.1:{port}/v1/providers/"),
            "modules.v1": format!("http://127.0.0.1:{port}/v1/modules/")
        })))
        .expect(1)
        .mount(&server)
        .await;

    let reg = Registry::new(&reg_host);
    let http = reqwest::Client::new();
    let pr = reg.provider(&http).await.expect("provider");
    let mr = reg.module(&http).await.expect("module");
    assert!(pr.base_url().as_str().contains("/v1/providers"));
    assert!(mr.base_url().as_str().contains("/v1/modules"));
}

#[tokio::test]
async fn module_builtin_skips_well_known() {
    let reg = Registry::new("registry.terraform.io");
    let http = reqwest::Client::new();
    let mr = reg.module(&http).await.expect("module");
    assert_eq!(
        mr.base_url().as_str(),
        "https://registry.terraform.io/v1/modules"
    );
}
