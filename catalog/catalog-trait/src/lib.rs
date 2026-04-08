mod catalog;
mod management;
mod populate;
mod provider_mirror;

pub mod availability;
pub mod read;
pub mod types;

pub use availability::CatalogAvailability;
pub use catalog::Catalog;
pub use management::CatalogManagement;
pub use populate::CatalogPopulate;
pub use provider_mirror::{CatalogProviderMirrorPopulate, CatalogProviderMirrorResolve};
pub use read::CatalogRead;

// Re-export env_defs types used by catalog traits so implementors (e.g. catalog-aws)
// don't need to depend on env_defs directly.
pub use env_defs::{
    ModuleManifest, ModuleResp, ModuleStackData, ProviderManifest, ProviderResp, StackManifest,
    StackMetadata, StackSpec, TfLockProvider, TfOutput, TfRequiredProvider, TfVariable,
};
