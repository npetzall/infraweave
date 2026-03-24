//! [`CatalogPopulate`](catalog_trait::CatalogPopulate) for [`MemCatalog`](crate::MemCatalog).

use async_trait::async_trait;
use catalog_trait::types::{CatalogRef, Metadata, TerraformInterface};
use catalog_trait::{
    CatalogPopulate, ModuleManifest, ModuleStackData, ProviderManifest, StackManifest,
};

use crate::MemCatalog;

#[async_trait]
impl CatalogPopulate for MemCatalog {
    async fn add_provider(
        &self,
        metadata: &Metadata,
        manifest: &ProviderManifest,
        terraform: &TerraformInterface,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        Ok(self.store.insert_provider(
            metadata.clone(),
            manifest.clone(),
            terraform.clone(),
            content.to_vec(),
        ))
    }

    async fn add_module(
        &self,
        metadata: &Metadata,
        manifest: &ModuleManifest,
        terraform: &TerraformInterface,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        Ok(self.store.insert_module(
            metadata.clone(),
            manifest.clone(),
            terraform.clone(),
            content.to_vec(),
        ))
    }

    async fn add_stack(
        &self,
        metadata: &Metadata,
        manifest: &StackManifest,
        terraform: &TerraformInterface,
        stack_data: Option<ModuleStackData>,
        content: &[u8],
    ) -> anyhow::Result<CatalogRef> {
        Ok(self.store.insert_stack(
            metadata.clone(),
            manifest.clone(),
            terraform.clone(),
            stack_data,
            content.to_vec(),
        ))
    }

    async fn add_attachment(
        &self,
        reference: &CatalogRef,
        name: &str,
        content: &[u8],
    ) -> anyhow::Result<()> {
        self.store
            .insert_attachment(&reference.id, name, content.to_vec())
    }
}
