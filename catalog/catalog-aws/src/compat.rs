//! Adapters between `catalog_trait::read` types and internal [`crate::compat_models`] (legacy serde shapes).

use serde::Serialize;

use catalog_trait::read::{Module, Provider, Stack};
use catalog_trait::{
    ModuleManifest, ModuleResp as CatalogModuleResp, ProviderManifest,
    ProviderResp as CatalogProviderResp, StackManifest,
};

use crate::compat_models::{ModuleResp as LegacyModuleResp, ProviderResp as LegacyProviderResp};

fn json_same_shape<T: Serialize, U: serde::de::DeserializeOwned>(v: &T) -> U {
    serde_json::from_value(serde_json::to_value(v).expect("serialize")).expect("compat JSON shape")
}

fn json_vec_same_shape<T: Serialize, U: serde::de::DeserializeOwned>(v: &[T]) -> Vec<U> {
    serde_json::from_value(serde_json::to_value(v).expect("serialize slice")).expect("compat vec")
}

fn json_opt_same_shape<T: Serialize, U: serde::de::DeserializeOwned>(v: &Option<T>) -> Option<U> {
    v.as_ref().map(|x| json_same_shape(x))
}

/// Convert DynamoDB legacy model into `env_defs` types re-exported by `catalog` (same JSON shape).
pub(crate) fn compat_provider_resp_into_catalog(r: LegacyProviderResp) -> CatalogProviderResp {
    serde_json::from_value(serde_json::to_value(&r).expect("serialize legacy ProviderResp"))
        .expect("legacy ProviderResp matches catalog_trait::ProviderResp")
}

pub(crate) fn compat_module_resp_into_catalog(r: LegacyModuleResp) -> CatalogModuleResp {
    serde_json::from_value(serde_json::to_value(&r).expect("serialize legacy ModuleResp"))
        .expect("legacy ModuleResp matches catalog_trait::ModuleResp")
}

/// Map [`Provider`] back to the legacy API payload shape (`ProviderResp`).
pub fn catalog_provider_to_legacy(p: &Provider) -> LegacyProviderResp {
    let m = p
        .metadata
        .as_ref()
        .expect("catalog_provider_to_legacy requires metadata");
    let manifest = p
        .manifest
        .as_ref()
        .map(legacy_provider_manifest_from_catalog)
        .expect("catalog_provider_to_legacy requires manifest");
    let tf = p.terraform.as_ref();
    LegacyProviderResp {
        name: m.name.clone(),
        version: m.version.clone(),
        timestamp: m.timestamp.clone(),
        description: m.description.clone(),
        reference: m.reference.clone(),
        manifest,
        tf_variables: tf
            .map(|t| json_vec_same_shape(&t.tf_variables))
            .unwrap_or_default(),
        tf_extra_environment_variables: tf
            .map(|t| t.tf_extra_environment_variables.clone())
            .unwrap_or_default(),
        s3_key: p.reference.id.clone(),
        deprecated: m.deprecated,
        deprecated_message: m.deprecated_message.clone(),
        yanked: false,
    }
}

fn legacy_provider_manifest_from_catalog(
    m: &ProviderManifest,
) -> crate::compat_models::ProviderManifest {
    serde_json::from_value(serde_json::to_value(m).expect("serialize catalog ProviderManifest"))
        .expect("ProviderManifest maps to legacy ProviderManifest")
}

/// Map [`Module`] to legacy `ModuleResp`.
pub fn catalog_module_to_legacy(m: &Module) -> LegacyModuleResp {
    let meta = m
        .metadata
        .as_ref()
        .expect("catalog_module_to_legacy requires metadata");
    let manifest: crate::compat_models::ModuleManifest = m
        .manifest
        .as_ref()
        .map(|man| {
            serde_json::from_value(serde_json::to_value(man).expect("serialize ModuleManifest"))
                .expect("ModuleManifest maps to legacy ModuleManifest")
        })
        .expect("catalog_module_to_legacy requires manifest");
    let tf = m.terraform.as_ref();
    LegacyModuleResp {
        track: meta.track.clone(),
        track_version: meta.version.clone(),
        version: meta.version.clone(),
        timestamp: meta.timestamp.clone(),
        module_name: manifest.spec.module_name.clone(),
        module: meta.name.clone(),
        module_type: meta.kind.clone(),
        description: meta.description.clone(),
        reference: meta.reference.clone(),
        manifest,
        tf_variables: tf
            .map(|t| json_vec_same_shape(&t.tf_variables))
            .unwrap_or_default(),
        tf_outputs: tf
            .map(|t| json_vec_same_shape(&t.tf_outputs))
            .unwrap_or_default(),
        tf_providers: tf
            .map(|t| json_vec_same_shape(&t.tf_providers))
            .unwrap_or_default(),
        tf_required_providers: tf
            .map(|t| json_vec_same_shape(&t.tf_required_providers))
            .unwrap_or_default(),
        tf_lock_providers: tf
            .map(|t| json_vec_same_shape(&t.tf_lock_providers))
            .unwrap_or_default(),
        tf_extra_environment_variables: tf
            .map(|t| t.tf_extra_environment_variables.clone())
            .unwrap_or_default(),
        s3_key: m.reference.id.clone(),
        oci_artifact_set: None,
        stack_data: None,
        version_diff: None,
        cpu: meta.cpu.clone(),
        memory: meta.memory.clone(),
        deprecated: meta.deprecated,
        deprecated_message: meta.deprecated_message.clone(),
        yanked: false,
        provider_mirror: m.provider_mirror.clone(),
    }
}

/// Map [`Stack`] to legacy `ModuleResp` (stacks are stored and served in the module response shape).
pub fn catalog_stack_to_legacy(s: &Stack) -> LegacyModuleResp {
    let meta = s
        .metadata
        .as_ref()
        .expect("catalog_stack_to_legacy requires metadata");
    let stack_man = s
        .manifest
        .as_ref()
        .expect("catalog_stack_to_legacy requires manifest");
    let module_manifest = stack_manifest_to_module_manifest(stack_man);
    let manifest: crate::compat_models::ModuleManifest = serde_json::from_value(
        serde_json::to_value(&module_manifest).expect("serialize ModuleManifest"),
    )
    .expect("ModuleManifest maps to legacy ModuleManifest");
    let tf = s.terraform.as_ref();
    LegacyModuleResp {
        track: meta.track.clone(),
        track_version: meta.version.clone(),
        version: meta.version.clone(),
        timestamp: meta.timestamp.clone(),
        module_name: manifest.spec.module_name.clone(),
        module: meta.name.clone(),
        module_type: meta.kind.clone(),
        description: meta.description.clone(),
        reference: meta.reference.clone(),
        manifest,
        tf_variables: tf
            .map(|t| json_vec_same_shape(&t.tf_variables))
            .unwrap_or_default(),
        tf_outputs: tf
            .map(|t| json_vec_same_shape(&t.tf_outputs))
            .unwrap_or_default(),
        tf_providers: tf
            .map(|t| json_vec_same_shape(&t.tf_providers))
            .unwrap_or_default(),
        tf_required_providers: tf
            .map(|t| json_vec_same_shape(&t.tf_required_providers))
            .unwrap_or_default(),
        tf_lock_providers: tf
            .map(|t| json_vec_same_shape(&t.tf_lock_providers))
            .unwrap_or_default(),
        tf_extra_environment_variables: tf
            .map(|t| t.tf_extra_environment_variables.clone())
            .unwrap_or_default(),
        s3_key: s.reference.id.clone(),
        oci_artifact_set: None,
        stack_data: json_opt_same_shape(&s.stack_data),
        version_diff: None,
        cpu: meta.cpu.clone(),
        memory: meta.memory.clone(),
        deprecated: meta.deprecated,
        deprecated_message: meta.deprecated_message.clone(),
        yanked: false,
        provider_mirror: s.provider_mirror.clone(),
    }
}

/// Same projection as [`crate::write::stack_manifest_to_module_manifest`]; source: `catalog-aws/src/write.rs`.
fn stack_manifest_to_module_manifest(s: &StackManifest) -> ModuleManifest {
    let json = serde_json::json!({
        "metadata": { "name": s.metadata.name },
        "apiVersion": s.api_version,
        "kind": s.kind,
        "spec": {
            "moduleName": s.spec.stack_name,
            "version": s.spec.version,
            "description": s.spec.description,
            "reference": s.spec.reference,
            "examples": s.spec.examples,
            "cpu": s.spec.cpu,
            "memory": s.spec.memory,
            "providers": []
        }
    });
    serde_json::from_value(json).expect("StackManifest to ModuleManifest")
}
