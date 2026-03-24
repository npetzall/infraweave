use super::{CatalogManagement, CatalogPopulate, CatalogRead};

/// Full catalog capability surface (read + populate + management).
pub trait Catalog: CatalogRead + CatalogPopulate + CatalogManagement {}

impl<T> Catalog for T where T: CatalogRead + CatalogPopulate + CatalogManagement {}
