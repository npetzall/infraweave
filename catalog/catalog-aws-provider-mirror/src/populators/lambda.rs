use std::collections::HashSet;

use async_trait::async_trait;
use aws_sdk_lambda::primitives::Blob;
use aws_sdk_lambda::types::InvocationType;
use aws_sdk_lambda::Client as LambdaClient;
use catalog_trait::{CatalogProviderMirrorPopulate, TfLockProvider};
use serde::{Deserialize, Serialize};

/// JSON body for async `InvokeFunction` (`Event`) to the mirror worker Lambda. Platforms and registry host come from worker env.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InvokePayload {
    /// Optional tracing / correlation id for logs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub providers: Vec<TfLockProvider>,
}

/// [`CatalogProviderMirrorPopulate`] that only async-invokes the mirror Lambda (`Event`).
#[derive(Clone)]
pub struct LambdaProviderMirrorPopulate {
    lambda: LambdaClient,
    arn: String,
}

impl LambdaProviderMirrorPopulate {
    const ASYNC_LAMBDA_INVOKE_PAYLOAD_MAX_BYTES: usize = 256 * 1024;
    const ASYNC_INVOKE_SAFETY_MARGIN_BYTES: usize = 4096;

    pub fn new(lambda: LambdaClient, function_arn: impl Into<String>) -> Self {
        let arn = function_arn.into().trim().to_string();
        Self { lambda, arn }
    }

    fn default_async_invoke_payload_limit_bytes(&self) -> usize {
        Self::ASYNC_LAMBDA_INVOKE_PAYLOAD_MAX_BYTES
            .saturating_sub(Self::ASYNC_INVOKE_SAFETY_MARGIN_BYTES)
    }

    fn dedupe_tf_lock_providers(&self, providers: &[TfLockProvider]) -> Vec<TfLockProvider> {
        let mut seen: HashSet<(&str, &str)> = HashSet::new();
        let mut out = Vec::new();
        for p in providers {
            let k = (p.source.as_str(), p.version.as_str());
            if seen.insert(k) {
                out.push(p.clone());
            }
        }
        out
    }

    fn mirror_payload_json_len(
        &self,
        correlation_id: Option<&str>,
        providers: &[TfLockProvider],
    ) -> usize {
        let payload = InvokePayload {
            correlation_id: correlation_id.map(String::from),
            providers: providers.to_vec(),
        };
        serde_json::to_vec(&payload)
            .map(|v| v.len())
            .unwrap_or(Self::ASYNC_LAMBDA_INVOKE_PAYLOAD_MAX_BYTES)
    }

    fn chunk_mirror_invoke_payloads(
        &self,
        correlation_id: Option<String>,
        providers: Vec<TfLockProvider>,
        max_serialized_bytes: usize,
    ) -> Vec<InvokePayload> {
        if providers.is_empty() {
            return vec![];
        }

        let cid = correlation_id.as_deref();
        let mut chunks: Vec<Vec<TfLockProvider>> = Vec::new();
        let mut current: Vec<TfLockProvider> = Vec::new();

        for p in providers {
            if current.is_empty() {
                current.push(p);
                continue;
            }

            let mut trial = current.clone();
            trial.push(p.clone());
            if self.mirror_payload_json_len(cid, &trial) > max_serialized_bytes {
                chunks.push(std::mem::take(&mut current));
                current.push(p);
            } else {
                current.push(p);
            }
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
            .into_iter()
            .map(|providers| InvokePayload {
                correlation_id: correlation_id.clone(),
                providers,
            })
            .collect()
    }

    fn provider_mirror_invoke_payloads(
        &self,
        providers: &[TfLockProvider],
        correlation_id: Option<String>,
    ) -> Option<Vec<InvokePayload>> {
        let deduped = self.dedupe_tf_lock_providers(providers);
        if deduped.is_empty() {
            return None;
        }
        let chunks = self.chunk_mirror_invoke_payloads(
            correlation_id,
            deduped,
            self.default_async_invoke_payload_limit_bytes(),
        );
        if chunks.is_empty() {
            return None;
        }
        Some(chunks)
    }
}

#[async_trait]
impl CatalogProviderMirrorPopulate for LambdaProviderMirrorPopulate {
    async fn ensure_providers_mirrored(&self, providers: &[TfLockProvider]) -> anyhow::Result<()> {
        if self.arn.is_empty() {
            return Ok(());
        }
        let arn = self.arn.as_str();
        let Some(chunks) = self.provider_mirror_invoke_payloads(providers, None) else {
            return Ok(());
        };

        let per_chunk: Vec<usize> = chunks.iter().map(|c| c.providers.len()).collect();
        log::info!(
            "provider_mirror_invoke: target_arn={} chunk_count={} provider_counts_per_chunk={:?}",
            arn,
            chunks.len(),
            per_chunk
        );

        for chunk in chunks {
            let payload = match serde_json::to_vec(&chunk) {
                Ok(b) => b,
                Err(e) => {
                    log::error!(
                        "provider_mirror_invoke: serialize InvokePayload failed: {}",
                        e
                    );
                    continue;
                }
            };

            if let Err(e) = self
                .lambda
                .invoke()
                .function_name(arn)
                .invocation_type(InvocationType::Event)
                .payload(Blob::new(payload))
                .send()
                .await
            {
                log::error!(
                    "provider_mirror_invoke: InvokeFunction Event failed ({} providers in chunk): {}",
                    chunk.providers.len(),
                    e
                );
            }
        }
        Ok(())
    }
}
