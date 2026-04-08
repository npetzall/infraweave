//! [`CatalogProviderMirrorPopulate`] implementations for the AWS provider mirror.

mod aws;
mod lambda;
mod noop;

pub use aws::{AwsProviderMirrorPopulator, MirrorRunStats};
#[cfg(feature = "lambda")]
pub use lambda::InvokePayload;
pub use lambda::LambdaProviderMirrorPopulate;
pub use noop::NoopProviderMirrorPopulate;
