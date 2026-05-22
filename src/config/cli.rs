//! Command-line argument parsing (clap derive). The CLI surface is
//! intentionally minimal — operators configure bithound through the
//! TOML file. The CLI exposes only the knobs needed to point the
//! sidecar at the right file or dry-run a config change.

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "bithound",
    about = "Observability sidecar for Bitcoin infrastructure"
)]
pub struct Cli {
    /// Path to bithound.toml. Falls back to ./bithound.toml then
    /// /etc/bithound/bithound.toml if not provided.
    #[arg(long, short)]
    pub config: Option<PathBuf>,

    /// Parse + validate the config (including env-var presence
    /// checks), print the merged result with secrets redacted, and
    /// exit 0. Use this to verify a config change before restarting.
    #[arg(long)]
    pub check_config: bool,

    /// Print version and exit.
    #[arg(long)]
    pub version: bool,
}
