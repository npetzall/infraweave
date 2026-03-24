//! HTTP [`catalog_trait::Catalog`] client for the REST surface implemented by [`catalog_http`]
//! (`/catalog/v1/...`), including deployments fronted by API Gateway.
//!
//! Download and attachment routes are keyed by **track / name / version**, while the trait API uses
//! [`catalog_trait::CatalogRef`]. This client keeps a small in-memory map from `reference.id` to
//! those coordinates, populated from [`CatalogRead::get`] and [`CatalogRead::list`] responses.
//! Call one of those through this client before `download_*` / attachment calls, and ensure list
//! queries include **metadata** in the projection when you rely on list-driven caching.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use catalog_trait::read::{
    CatalogEntry, ContentSource, Module, Page, ProjectionFields, Provider, Query, Stack,
};
use catalog_trait::types::{
    CatalogKind, CatalogRef, Metadata, TerraformInterface, VersionSelector,
};
use catalog_trait::{
    CatalogManagement, CatalogPopulate, CatalogRead, ModuleManifest, ModuleStackData,
    ProviderManifest, StackManifest,
};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use reqwest::header::CONTENT_TYPE;
use serde::de::DeserializeOwned;
use serde::Serialize;

#[derive(Clone, Debug)]
struct EntryCoords {
    kind: CatalogKind,
    track: String,
    name: String,
    version: String,
}

/// [`Catalog`] implementation backed by catalog-http JSON + artifact endpoints.
#[derive(Clone, Debug)]
pub struct AwsHttpCatalog {
    client: reqwest::Client,
    base_url: String,
    coords: Arc<Mutex<HashMap<String, EntryCoords>>>,
}

impl AwsHttpCatalog {
    /// `base_url` is the service origin only, e.g. `https://example.com` (no `/catalog` suffix).
    pub fn new(base_url: impl Into<String>) -> Self {
        Self::with_client(reqwest::Client::new(), base_url)
    }

    pub fn with_client(client: reqwest::Client, base_url: impl Into<String>) -> Self {
        Self {
            client,
            base_url: base_url.into(),
            coords: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn root(&self) -> String {
        format!("{}/catalog/v1", self.base_url.trim_end_matches('/'))
    }

    fn enc(s: &str) -> String {
        utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
    }

    fn kind_path(kind: CatalogKind) -> &'static str {
        match kind {
            CatalogKind::Provider => "provider",
            CatalogKind::Module => "module",
            CatalogKind::Stack => "stack",
        }
    }

    fn version_segment(v: &VersionSelector) -> String {
        match v {
            VersionSelector::Latest => "latest".to_string(),
            VersionSelector::Exact(s) => s.clone(),
        }
    }

    fn projection_to_wire(mask: ProjectionFields) -> String {
        let mut parts = Vec::new();
        if mask.contains(ProjectionFields::METADATA) {
            parts.push("metadata");
        }
        if mask.contains(ProjectionFields::MANIFEST) {
            parts.push("manifest");
        }
        if mask.contains(ProjectionFields::TERRAFORM) {
            parts.push("terraform");
        }
        if mask.contains(ProjectionFields::STACK_DATA) {
            parts.push("stack_data");
        }
        if mask.contains(ProjectionFields::PROVIDER_MIRROR) {
            parts.push("provider_mirror");
        }
        parts.join(",")
    }

    fn remember_coords(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        metadata: Option<&Metadata>,
    ) {
        let Some(m) = metadata else {
            return;
        };
        let mut g = self.coords.lock().expect("coords mutex poisoned");
        g.insert(
            reference.id.clone(),
            EntryCoords {
                kind,
                track: m.track.clone(),
                name: m.name.clone(),
                version: m.version.clone(),
            },
        );
    }

    fn remember_entry(&self, entry: &CatalogEntry) {
        match entry {
            CatalogEntry::Provider(p) => {
                self.remember_coords(CatalogKind::Provider, &p.reference, p.metadata.as_ref());
            }
            CatalogEntry::Module(m) => {
                self.remember_coords(CatalogKind::Module, &m.reference, m.metadata.as_ref());
            }
            CatalogEntry::Stack(s) => {
                self.remember_coords(CatalogKind::Stack, &s.reference, s.metadata.as_ref());
            }
        }
    }

    fn coords_for(&self, reference: &CatalogRef) -> anyhow::Result<EntryCoords> {
        self.coords
            .lock()
            .expect("coords mutex poisoned")
            .get(&reference.id)
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "AwsHttpCatalog: unknown reference id `{}`; call get/list through this client first (list entries need metadata in the projection to be cached)",
                    reference.id
                )
            })
    }

    fn apply_list_query<'a>(
        &self,
        mut req: reqwest::RequestBuilder,
        query: &'a Query,
    ) -> reqwest::RequestBuilder {
        if let Some(ref n) = query.name {
            if !n.is_empty() {
                req = req.query(&[("name", n.as_str())]);
            }
        }
        if let Some(ref t) = query.track {
            if !t.is_empty() {
                req = req.query(&[("track", t.as_str())]);
            }
        }
        if let Some(limit) = query.limit {
            req = req.query(&[("limit", limit.to_string())]);
        }
        if let Some(ref n) = query.next {
            if !n.is_empty() {
                req = req.query(&[("next", n.as_str())]);
            }
        }
        if let Some(p) = query.projection {
            if p != ProjectionFields::ALL && p != ProjectionFields::default() {
                let s = Self::projection_to_wire(p);
                if !s.is_empty() {
                    req = req.query(&[("projection", s.as_str())]);
                }
            }
        }
        req
    }

    async fn http_fail_context(res: reqwest::Response) -> anyhow::Error {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        anyhow::anyhow!("catalog HTTP {status}: {body}")
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
    ) -> anyhow::Result<T> {
        let res = req.send().await?;
        if res.status().is_success() {
            return Ok(res.json().await?);
        }
        Err(Self::http_fail_context(res).await)
    }

    async fn send_empty_ok(&self, req: reqwest::RequestBuilder) -> anyhow::Result<()> {
        let res = req.send().await?;
        if res.status().is_success() {
            return Ok(());
        }
        Err(Self::http_fail_context(res).await)
    }

    async fn fetch_download_body(&self, url: String) -> anyhow::Result<ContentSource> {
        let res = self.client.get(&url).send().await?;
        if !res.status().is_success() {
            return Err(Self::http_fail_context(res).await);
        }
        let ct = res
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ct.contains("application/json") {
            let v: serde_json::Value = res.json().await?;
            let u = v.get("url").and_then(|x| x.as_str()).ok_or_else(|| {
                anyhow::anyhow!("catalog download JSON missing string `url` field")
            })?;
            return Ok(ContentSource::Url(u.to_string()));
        }
        Ok(ContentSource::Bytes(res.bytes().await?.to_vec()))
    }
}

#[async_trait]
impl CatalogRead for AwsHttpCatalog {
    async fn list(&self, kind: CatalogKind, query: &Query) -> anyhow::Result<Page<CatalogEntry>> {
        let path = match kind {
            CatalogKind::Provider => "providers",
            CatalogKind::Module => "modules",
            CatalogKind::Stack => "stacks",
        };
        let url = format!("{}/{}", self.root(), path);
        let req = self.apply_list_query(self.client.get(&url), query);
        match kind {
            CatalogKind::Provider => {
                let page: Page<Provider> = self.get_json(req).await?;
                for p in &page.items {
                    self.remember_coords(CatalogKind::Provider, &p.reference, p.metadata.as_ref());
                }
                Ok(Page {
                    items: page.items.into_iter().map(CatalogEntry::Provider).collect(),
                    next: page.next,
                })
            }
            CatalogKind::Module => {
                let page: Page<Module> = self.get_json(req).await?;
                for m in &page.items {
                    self.remember_coords(CatalogKind::Module, &m.reference, m.metadata.as_ref());
                }
                Ok(Page {
                    items: page.items.into_iter().map(CatalogEntry::Module).collect(),
                    next: page.next,
                })
            }
            CatalogKind::Stack => {
                let page: Page<Stack> = self.get_json(req).await?;
                for s in &page.items {
                    self.remember_coords(CatalogKind::Stack, &s.reference, s.metadata.as_ref());
                }
                Ok(Page {
                    items: page.items.into_iter().map(CatalogEntry::Stack).collect(),
                    next: page.next,
                })
            }
        }
    }

    async fn get(
        &self,
        kind: CatalogKind,
        name: &str,
        track: &str,
        version: VersionSelector,
    ) -> anyhow::Result<Option<CatalogEntry>> {
        let ver = Self::version_segment(&version);
        let url = format!(
            "{}/{}/{}/{}/{}",
            self.root(),
            Self::kind_path(kind),
            Self::enc(track),
            Self::enc(name),
            Self::enc(&ver)
        );
        let res = self.client.get(&url).send().await?;
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !res.status().is_success() {
            return Err(Self::http_fail_context(res).await);
        }
        let entry = match kind {
            CatalogKind::Provider => {
                let p: Provider = res.json().await?;
                let entry = CatalogEntry::Provider(p);
                self.remember_entry(&entry);
                entry
            }
            CatalogKind::Module => {
                let m: Module = res.json().await?;
                let entry = CatalogEntry::Module(m);
                self.remember_entry(&entry);
                entry
            }
            CatalogKind::Stack => {
                let s: Stack = res.json().await?;
                let entry = CatalogEntry::Stack(s);
                self.remember_entry(&entry);
                entry
            }
        };
        Ok(Some(entry))
    }

    async fn download_provider(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        let c = self.coords_for(reference)?;
        if c.kind != CatalogKind::Provider {
            anyhow::bail!("reference {} is not a provider", reference.id);
        }
        let url = format!(
            "{}/provider/{}/{}/{}/download",
            self.root(),
            Self::enc(&c.track),
            Self::enc(&c.name),
            Self::enc(&c.version)
        );
        self.fetch_download_body(url).await
    }

    async fn download_module(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        let c = self.coords_for(reference)?;
        if c.kind != CatalogKind::Module {
            anyhow::bail!("reference {} is not a module", reference.id);
        }
        let url = format!(
            "{}/module/{}/{}/{}/download",
            self.root(),
            Self::enc(&c.track),
            Self::enc(&c.name),
            Self::enc(&c.version)
        );
        self.fetch_download_body(url).await
    }

    async fn download_stack(&self, reference: &CatalogRef) -> anyhow::Result<ContentSource> {
        let c = self.coords_for(reference)?;
        if c.kind != CatalogKind::Stack {
            anyhow::bail!("reference {} is not a stack", reference.id);
        }
        let url = format!(
            "{}/stack/{}/{}/{}/download",
            self.root(),
            Self::enc(&c.track),
            Self::enc(&c.name),
            Self::enc(&c.version)
        );
        self.fetch_download_body(url).await
    }

    async fn list_attachments(&self, reference: &CatalogRef) -> anyhow::Result<Vec<String>> {
        let c = self.coords_for(reference)?;
        let url = format!(
            "{}/{}/{}/{}/{}/attachments",
            self.root(),
            Self::kind_path(c.kind),
            Self::enc(&c.track),
            Self::enc(&c.name),
            Self::enc(&c.version)
        );
        self.get_json(self.client.get(url)).await
    }

    async fn download_attachment(
        &self,
        reference: &CatalogRef,
        name: &str,
    ) -> anyhow::Result<ContentSource> {
        let c = self.coords_for(reference)?;
        let url = format!(
            "{}/{}/{}/{}/{}/attachments/{}",
            self.root(),
            Self::kind_path(c.kind),
            Self::enc(&c.track),
            Self::enc(&c.name),
            Self::enc(&c.version),
            Self::enc(name)
        );
        self.fetch_download_body(url).await
    }
}

#[async_trait]
impl CatalogManagement for AwsHttpCatalog {
    async fn promote(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        track: &str,
        version: Option<&str>,
    ) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            reference: &'a CatalogRef,
            track: &'a str,
            version: Option<&'a str>,
        }
        let path = match kind {
            CatalogKind::Provider => "provider/promote",
            CatalogKind::Module => "module/promote",
            CatalogKind::Stack => "stack/promote",
        };
        let url = format!("{}/{}", self.root(), path);
        let body = Body {
            reference,
            track,
            version,
        };
        self.send_empty_ok(self.client.post(&url).json(&body)).await
    }

    async fn deprecate(
        &self,
        kind: CatalogKind,
        reference: &CatalogRef,
        reason: &str,
    ) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            reference: &'a CatalogRef,
            reason: &'a str,
        }
        let path = match kind {
            CatalogKind::Provider => "provider/deprecate",
            CatalogKind::Module => "module/deprecate",
            CatalogKind::Stack => "stack/deprecate",
        };
        let url = format!("{}/{}", self.root(), path);
        let body = Body { reference, reason };
        self.send_empty_ok(self.client.post(&url).json(&body)).await
    }

    async fn yank(&self, kind: CatalogKind, reference: &CatalogRef) -> anyhow::Result<()> {
        #[derive(Serialize)]
        struct Body<'a> {
            reference: &'a CatalogRef,
        }
        let path = match kind {
            CatalogKind::Provider => "provider/yank",
            CatalogKind::Module => "module/yank",
            CatalogKind::Stack => "stack/yank",
        };
        let url = format!("{}/{}", self.root(), path);
        let body = Body { reference };
        self.send_empty_ok(self.client.post(&url).json(&body)).await
    }
}

#[async_trait]
impl CatalogPopulate for AwsHttpCatalog {
    async fn add_provider(
        &self,
        _metadata: &Metadata,
        _manifest: &ProviderManifest,
        _terraform: &TerraformInterface,
        _content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        anyhow::bail!("AwsHttpCatalog: populate (add_provider) is not exposed by catalog-http")
    }

    async fn add_module(
        &self,
        _metadata: &Metadata,
        _manifest: &ModuleManifest,
        _terraform: &TerraformInterface,
        _content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        anyhow::bail!("AwsHttpCatalog: populate (add_module) is not exposed by catalog-http")
    }

    async fn add_stack(
        &self,
        _metadata: &Metadata,
        _manifest: &StackManifest,
        _terraform: &TerraformInterface,
        _stack_data: Option<ModuleStackData>,
        _content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        anyhow::bail!("AwsHttpCatalog: populate (add_stack) is not exposed by catalog-http")
    }

    async fn add_attachment(
        &self,
        _reference: &CatalogRef,
        _name: &str,
        _content: &[u8],
    ) -> anyhow::Result<()> {
        anyhow::bail!("AwsHttpCatalog: populate (add_attachment) is not exposed by catalog-http")
    }
}
