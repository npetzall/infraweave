use env_defs::{ProviderResp, TfLockProvider, TfOutput, TfRequiredProvider, TfVariable};
use serde::{Deserialize, Serialize};

/// Opaque reference to a stored catalog entry (provider/module/stack version).
///
/// Implementations are free to interpret this as a composite key,
/// versioned identifier, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogRef {
    pub id: String,
}

/// Single metadata struct for providers, modules and stacks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub name: String,
    pub kind: String,
    pub track: String,
    pub version: String,
    pub timestamp: String,
    pub description: String,
    pub reference: String,
    pub cpu: String,
    pub memory: String,
    pub deprecated: bool,
    pub deprecated_message: Option<String>,
}

/// Unified Terraform-related interface data used by providers, modules and stacks.
///
/// Some fields may be unused for certain kinds (e.g. providers might not
/// have outputs); in those cases the corresponding vectors can simply be left empty.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TerraformInterface {
    pub tf_variables: Vec<TfVariable>,
    pub tf_outputs: Vec<TfOutput>,
    pub tf_providers: Vec<ProviderResp>,
    pub tf_required_providers: Vec<TfRequiredProvider>,
    pub tf_lock_providers: Vec<TfLockProvider>,
    pub tf_extra_environment_variables: Vec<String>,
}

/// What kind of catalog entry to operate on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CatalogKind {
    Provider,
    Module,
    Stack,
}

/// How to select a version when fetching from the catalog.
///
/// JSON (externally tagged): `{"Latest":null}` or `{"Exact":"1.2.3"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VersionSelector {
    /// Use the latest known version for the given name/track.
    Latest,
    /// Use this exact semantic version (or whatever versioning scheme you use).
    Exact(String),
}
