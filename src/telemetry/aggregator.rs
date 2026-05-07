pub trait Aggregator {
    type Observation;
    type Snapshot: Clone + Send + Sync + 'static;

    fn apply(&mut self, observation: Self::Observation);
    fn snapshot(&self) -> Self::Snapshot;
}
