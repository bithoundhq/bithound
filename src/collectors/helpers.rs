//! Cross-collector free functions.
//!
//! These four helpers are the small bits of plumbing every polling
//! collector needs but that don't belong on the collector struct
//! itself: stamping the wall-clock window around a future, producing
//! the empty-attributes value that most observations carry, and
//! turning chrono `DateTime`s into the `ProbeWindow` / `latency_ms`
//! the observation envelope expects without crashing on a backwards
//! clock jump.

use std::collections::BTreeMap;
use std::future::Future;

use chrono::{DateTime, Utc};

use crate::observations::{Attributes, ProbeWindow};

/// Run `future` to completion, returning `(start, end, output)`
/// stamped from `Utc::now()`. Used by polling collectors to time
/// individual RPC calls — the timestamps feed `safe_probe_window`
/// and `duration_ms` below.
pub(crate) async fn timed<F: Future>(future: F) -> (DateTime<Utc>, DateTime<Utc>, F::Output) {
    let start = Utc::now();
    let result = future.await;
    let end = Utc::now();
    (start, end, result)
}

/// Empty `Attributes` value. Most observations leave the
/// attribute bag empty; this saves the import and the
/// `BTreeMap::new()` ceremony at every call site.
pub(crate) fn empty_attrs() -> Attributes {
    Attributes(BTreeMap::new())
}

/// Construct a `ProbeWindow` defensively. A backwards clock jump
/// between `started_at` and `completed_at` collapses to a zero-width
/// window pinned at the later instant instead of panicking — matches
/// the failure path's old fallback so the success path doesn't crash
/// the poll task on NTP correction.
pub(crate) fn safe_probe_window(
    started_at: DateTime<Utc>,
    completed_at: DateTime<Utc>,
) -> ProbeWindow {
    ProbeWindow::new(started_at, completed_at)
        .unwrap_or_else(|_| ProbeWindow::new(completed_at, completed_at).unwrap())
}

/// Wall-clock delta between two `DateTime<Utc>` as milliseconds.
/// Returns `None` if the clock jumped backwards rather than
/// surfacing a negative latency.
pub(crate) fn duration_ms(from: DateTime<Utc>, to: DateTime<Utc>) -> Option<u64> {
    let ms = (to - from).num_milliseconds();
    if ms < 0 {
        None
    } else {
        Some(ms as u64)
    }
}
