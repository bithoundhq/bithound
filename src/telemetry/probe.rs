use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::rpc::RpcError;

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error("Transport error: {0}")]
    Transport(String),
    #[error("Decode: {0}")]
    Decode(String),
}

/// A Probe that observes a given value of type Output.
#[async_trait]
pub trait Probe: Send + Sync + std::fmt::Debug + 'static {
    type Output: Send + Sync + 'static;

    async fn observe(&self) -> Result<Self::Output, ProbeError>;
}

#[async_trait]
pub trait ProbeEventStream {
    type Event: Send + 'static;

    async fn next_event(&mut self) -> Option<Self::Event>;
    fn abort(self);
}

#[derive(Debug, Clone)]
pub enum ProbeObservation<T> {
    Success {
        value: T,
        observed_at: DateTime<Utc>,
        latency: Duration,
    },
    Error {
        description: String,
        observed_at: DateTime<Utc>,
        latency: Duration,
    },
    Timeout {
        ttl: tokio::time::Duration,
        observed_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone)]
pub struct ProbeConfig {
    pub interval: tokio::time::Duration,
    pub timeout: tokio::time::Duration,
}

#[derive(Debug)]
pub struct ProbeRunner {
    handle: JoinHandle<()>,
}

impl ProbeRunner {
    /// Runs a given Probe with the specified ProbeConfig.
    /// Converts a `Result<P::Output, ProbeError> into a ProbeObservation
    /// and sends over to the mpsc channel.
    pub fn run_probe<P, ProbeEvent>(
        probe: P,
        config: ProbeConfig,
        sender: mpsc::Sender<ProbeEvent>,
    ) -> Self
    where
        P: Probe + Send + Sync + 'static,
        P::Output: Send + 'static,
        ProbeEvent: From<ProbeObservation<P::Output>> + Send + Sync + 'static,
    {
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.interval);

            loop {
                interval.tick().await;

                let observed_at = Utc::now();

                let result = tokio::time::timeout(config.timeout, probe.observe()).await;

                let finished_at = Utc::now();

                let output = match result {
                    Ok(Ok(output)) => ProbeObservation::Success {
                        value: output,
                        observed_at,
                        latency: finished_at - observed_at,
                    },
                    Ok(Err(e)) => ProbeObservation::Error {
                        description: e.to_string(),
                        observed_at,
                        latency: finished_at - observed_at,
                    },
                    Err(_e) => ProbeObservation::Timeout {
                        ttl: config.timeout,
                        observed_at,
                    },
                };

                if let Err(_) = sender.send(ProbeEvent::from(output)).await {
                    tracing::debug!(?probe, "probe receiver dropped; stopping runner");
                    break;
                }
            }
        });

        Self { handle }
    }

    pub fn abort(self) {
        self.handle.abort();
    }

    pub async fn join(self) -> Result<(), tokio::task::JoinError> {
        self.handle.await
    }
}
