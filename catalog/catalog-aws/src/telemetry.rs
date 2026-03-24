//! Lightweight telemetry points for catalog operations.
//!
//! Tracks: operation name, item kind, outcome (success/failure), latency bucket.

use std::time::Duration;

use catalog_trait::types::CatalogKind;

/// Outcome of an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Failure,
}

/// Coarse latency bucket for operational dashboards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatencyBucket {
    /// < 10ms
    Fast,
    /// 10ms - 100ms
    Normal,
    /// 100ms - 1s
    Slow,
    /// >= 1s
    VerySlow,
}

impl LatencyBucket {
    pub fn from_duration(d: Duration) -> Self {
        let ms = d.as_millis() as u64;
        if ms < 10 {
            LatencyBucket::Fast
        } else if ms < 100 {
            LatencyBucket::Normal
        } else if ms < 1000 {
            LatencyBucket::Slow
        } else {
            LatencyBucket::VerySlow
        }
    }
}

/// Record a catalog operation for telemetry.
///
/// Logs structured data; can be extended to emit metrics.
pub fn record_operation(
    operation: &str,
    kind: Option<CatalogKind>,
    outcome: Outcome,
    latency: Duration,
) {
    let kind_str = kind
        .map(|k| format!("{:?}", k))
        .unwrap_or_else(|| "unknown".to_string());
    let outcome_str = match outcome {
        Outcome::Success => "success",
        Outcome::Failure => "failure",
    };
    let bucket = LatencyBucket::from_duration(latency);

    log::info!(
        "catalog operation operation={} item_kind={} outcome={} latency_bucket={:?} latency_ms={}",
        operation,
        kind_str,
        outcome_str,
        bucket,
        latency.as_millis()
    );
}

/// Wrapper to measure operation latency and record telemetry.
pub fn with_telemetry<F, T, E>(operation: &str, kind: Option<CatalogKind>, f: F) -> Result<T, E>
where
    F: FnOnce() -> Result<T, E>,
{
    let start = std::time::Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    let outcome = if result.is_ok() {
        Outcome::Success
    } else {
        Outcome::Failure
    };
    record_operation(operation, kind, outcome, elapsed);
    result
}
