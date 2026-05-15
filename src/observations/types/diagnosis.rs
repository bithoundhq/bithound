//! Types for diagnostic observations.
use serde::{Deserialize, Serialize};

use crate::observations::types::Confidence;
use crate::shared::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisObservation {
    pub diagnosis: DiagnosisName,
    pub summary: String,
    pub confidence: Confidence,
    pub likely_causes: Vec<String>,
    pub recommended_actions: Vec<String>,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DiagnosisName(pub String);
