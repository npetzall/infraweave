use crate::TfVariable;
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct ProviderManifest {
    pub metadata: Metadata,
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub spec: ProviderSpec,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Metadata {
    pub name: String,
}

// This struct represents the actual spec part of the manifest
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct ProviderSpec {
    pub provider: String,
    pub alias: Option<String>,
    pub version: Option<String>,
    pub description: String,
    pub reference: String,
}

impl ProviderSpec {
    pub fn configuration_name(&self) -> String {
        if let Some(alias) = &self.alias {
            return format!("{}.{}", self.provider, alias);
        } else {
            return self.provider.clone();
        }
    }
}

// Wrapped of the ProviderManifest to with some metadata
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct ProviderResp {
    pub name: String,
    pub version: String,
    pub timestamp: String,
    pub description: String,
    pub reference: String,
    pub manifest: ProviderManifest,
    #[serde(default)]
    pub tf_variables: Vec<TfVariable>,
    #[serde(default)]
    pub tf_extra_environment_variables: Vec<String>,
    pub s3_key: String,
    #[serde(default)]
    pub deprecated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_message: Option<String>,
    #[serde(default)]
    pub yanked: bool,
}
