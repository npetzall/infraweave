use anyhow::Result;
use catalog_aws::AwsCatalog;

pub async fn init_catalog() -> Result<AwsCatalog> {
    AwsCatalog::from_env().await
}
