use catalog_trait::read::ContentSource;

/// Load catalog binary content into memory.
///
/// - [`ContentSource::Bytes`] is returned as-is.
/// - [`ContentSource::Path`] is read with [`tokio::fs::read`].
/// - [`ContentSource::Url`] is fetched with a short-lived [`reqwest::Client`] (follows redirects).
pub async fn materialize_content(source: ContentSource) -> anyhow::Result<Vec<u8>> {
    match source {
        ContentSource::Bytes(b) => Ok(b),
        ContentSource::Path(p) => Ok(tokio::fs::read(&p).await?),
        ContentSource::Url(url) => {
            let client = reqwest::Client::builder()
                .use_rustls_tls()
                .build()
                .map_err(|e| anyhow::anyhow!("reqwest client: {e}"))?;
            let res = client.get(&url).send().await?.error_for_status()?;
            Ok(res.bytes().await?.to_vec())
        }
    }
}
