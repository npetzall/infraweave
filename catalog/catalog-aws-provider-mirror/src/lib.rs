//! AWS S3 Terraform provider mirror: [`CatalogProviderMirrorResolve`] for presigned mirror URLs.
//!
//! Resolution uses the **packed** layout only: `{source}/terraform-provider-{TYPE}_{VERSION}_{TARGET}.zip`
//! (same `source` string as in the lockfile). Objects are found with S3 `HEAD` and exposed via presigned GET;
//! there is no registry round-trip.
//!
//! Enable Cargo feature **`lambda`** to build the `bootstrap` Lambda binary (`lambda_runtime` + `tracing-subscriber`).

mod packed;
mod populators;
mod resolvers;
pub mod s3_util;

pub use populators::{
    AwsProviderMirrorPopulator, LambdaProviderMirrorPopulate, NoopProviderMirrorPopulate,
};
pub use resolvers::AwsProviderMirrorResolve;

pub use populators::MirrorRunStats;

#[cfg(feature = "lambda")]
pub use populators::InvokePayload;
