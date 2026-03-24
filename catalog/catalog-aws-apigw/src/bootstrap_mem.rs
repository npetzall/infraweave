use anyhow::Result;
use catalog_mem::MemCatalog;

pub fn init_catalog() -> Result<MemCatalog> {
    Ok(MemCatalog::default())
}
