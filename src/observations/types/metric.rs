//! Types for metric observations.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::shared::parse::{parse_dotted_name, ParseDottedNameError};

/// Metrics are numeric, histogram or summary observations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricObservation {
    pub name: MetricName,
    pub kind: MetricKind,
    pub value: MetricValue,
    pub unit: Unit,
}

impl MetricObservation {
    pub fn validate(&self) -> Result<()> {
        match (&self.kind, &self.value) {
            (MetricKind::Gauge, MetricValue::Numeric(_)) => Ok(()),
            (MetricKind::Delta, MetricValue::Numeric(_)) => Ok(()),
            (MetricKind::Counter, MetricValue::Numeric(NumericValue::U64(_))) => Ok(()),
            (MetricKind::Histogram, MetricValue::Histogram(_)) => Ok(()),
            (MetricKind::Summary, MetricValue::Summary(_)) => Ok(()),
            _ => Err(anyhow!("invalid metric value")),
        }
    }
}

/// Canonical name for a metric (e.g. `bitcoin.peer_count`).
///
/// Constructed only through [`MetricName::parse`] or
/// [`MetricName::from_well_known`]; the inner field is private so
/// callers can't bypass validation by wrapping arbitrary strings (per
/// ADR-D2).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MetricName(String);

impl MetricName {
    pub fn parse(s: impl AsRef<str>) -> Result<Self, ParseDottedNameError> {
        parse_dotted_name(s.as_ref()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Lift a `&'static str` known to satisfy the grammar. Debug-asserts
    /// the parse rule; release builds skip the check.
    pub fn from_well_known(name: &'static str) -> Self {
        debug_assert!(
            parse_dotted_name(name).is_ok(),
            "invalid well_known metric name: {name}"
        );
        MetricName(name.to_string())
    }
}

impl AsRef<str> for MetricName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MetricName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for MetricName {
    type Error = ParseDottedNameError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl From<MetricName> for String {
    fn from(n: MetricName) -> String {
        n.0
    }
}

#[cfg(test)]
mod metric_name_tests {
    use super::*;

    #[test]
    fn parse_accepts_valid() {
        assert_eq!(
            MetricName::parse("bitcoin.peer_count").unwrap().as_str(),
            "bitcoin.peer_count"
        );
    }

    #[test]
    fn parse_rejects_invalid() {
        assert!(MetricName::parse("peer_count").is_err());
        assert!(MetricName::parse("BadCase").is_err());
    }

    #[test]
    fn serde_revalidates() {
        let json = "\"peer_count\"";
        let err = serde_json::from_str::<MetricName>(json).unwrap_err();
        assert!(err.to_string().contains("at least one dot"));
    }

    #[test]
    fn serde_round_trips() {
        let n = MetricName::parse("bitcoin.peer_count").unwrap();
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(json, "\"bitcoin.peer_count\"");
        let back: MetricName = serde_json::from_str(&json).unwrap();
        assert_eq!(back, n);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricKind {
    Gauge,
    Counter,
    Delta,
    Histogram,
    Summary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MetricValue {
    Numeric(NumericValue),
    Histogram(HistogramValue),
    Summary(SummaryValue),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NumericValue {
    U64(u64),
    I64(i64),
    F64(f64),
}

impl From<u64> for MetricValue {
    fn from(value: u64) -> Self {
        MetricValue::Numeric(NumericValue::U64(value))
    }
}

impl From<i64> for MetricValue {
    fn from(value: i64) -> Self {
        MetricValue::Numeric(NumericValue::I64(value))
    }
}

impl From<f64> for MetricValue {
    fn from(value: f64) -> Self {
        MetricValue::Numeric(NumericValue::F64(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Dimensionless,

    Count,

    Bytes,
    VirtualBytes,
    WeightUnits,

    Seconds,
    Milliseconds,

    Satoshis,
    MilliSatoshis,

    Ratio,

    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistogramValue {
    pub buckets: Vec<HistogramBucket>,
    pub count: u64,
    pub sum: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistogramBucket {
    pub le: f64,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SummaryValue {
    pub quantiles: Vec<Quantile>,
    pub count: Option<u64>,
    pub sum: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quantile {
    pub quantile: f64,
    pub value: f64,
}
