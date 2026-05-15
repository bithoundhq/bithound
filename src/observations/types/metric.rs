//! Types for metric observations.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MetricName(pub String);

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
