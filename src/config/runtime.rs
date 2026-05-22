use serde::Deserialize;

/// Top-level `[runtime]` block. Single-writer pipeline knobs.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeConfig {
    /// Capacity of the bounded `mpsc::channel<ObservationBatch>` that
    /// connects collectors to the consumer. Backpressure shows up
    /// here when collectors outrun the consumer.
    #[serde(default = "default_channel_capacity")]
    pub channel_capacity: usize,

    /// How long the supervisor waits for in-flight work to finish
    /// after a SIGTERM before force-aborting.
    #[serde(default = "default_shutdown_deadline")]
    pub shutdown_deadline_seconds: u32,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            channel_capacity: default_channel_capacity(),
            shutdown_deadline_seconds: default_shutdown_deadline(),
        }
    }
}

fn default_channel_capacity() -> usize {
    1024
}
fn default_shutdown_deadline() -> u32 {
    30
}
