mod probe;
mod reducer;

pub use probe::*;
pub use reducer::*;

use tokio::sync::watch;

pub async fn run_state_loop<S, R>(
    mut stream: S,
    mut reducer: R,
    snapshot_tx: watch::Sender<R::Snapshot>,
) where
    S: ProbeEventStream,
    R: StateReducer<S::Event>,
{
    while let Some(event) = stream.next_event().await {
        reducer.apply(event);

        let snapshot = reducer.snapshot();

        if let Err(e) = snapshot_tx.send(snapshot) {
            tracing::warn!("No receivers are alive: {}", e);
        }
    }

    stream.abort();
}
