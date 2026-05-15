use chrono::{DateTime, Utc};

use crate::shared::types::ObservationId;

#[derive(Debug, Clone)]
pub struct Projected<T> {
    pub value: T,
    pub observation_id: ObservationId,
    pub observed_at: DateTime<Utc>,
}
