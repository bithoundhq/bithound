use async_trait::async_trait;

/// A StateReducer. Events from a ProbeEventStream are passed into it
/// to update the current world state of a given observed value.
#[async_trait]
pub trait StateReducer<E> {
    type Snapshot: Clone + Send + Sync + 'static;

    /// Applies changes to the state.
    fn apply(&mut self, event: E);

    /// Takes a snapshot of the current state.
    fn snapshot(&self) -> Self::Snapshot;
}

