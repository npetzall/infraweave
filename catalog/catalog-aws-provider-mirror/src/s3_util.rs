//! Small S3 helpers shared by the library resolver and the Lambda populate worker.

/// True when `head_object` failed because the object does not exist.
#[must_use]
pub fn head_object_is_not_found(
    err: &aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::head_object::HeadObjectError>,
) -> bool {
    match err {
        aws_sdk_s3::error::SdkError::ServiceError(se) => se.err().is_not_found(),
        _ => false,
    }
}
