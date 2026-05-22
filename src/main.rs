// Many types are defined ahead of the runtime that will use them — the
// runtime wiring (Phase 10+) is what actually drives them. Allow
// dead_code crate-wide until that lands; CLAUDE.md flags this as
// expected for the current phase.
#![allow(dead_code)]

mod collectors;
mod config;
mod diagnostics;
mod incidents;
mod notifications;
mod observations;
mod read_models;
mod rpc;
mod shared;
mod storage;

use clap::Parser;

use crate::config::{cli::Cli, Config};

const EX_CONFIG: i32 = 78;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if cli.version {
        println!("bithound {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    match Config::load_from_args_and_env(&cli).await {
        Ok(loaded) => {
            if cli.check_config {
                // Config carries env-var names, never secret values;
                // ResolvedSecrets uses SecretString which suppresses
                // Debug, so this dump is safe to show to operators.
                println!("{:#?}", loaded.config);
                return;
            }

            // Runtime hand-off lands in a later phase. For now, print
            // a one-line summary so operators can confirm the load
            // ran end-to-end.
            println!(
                "config loaded: sidecar {} | {} bitcoin nodes | {} collectors | {} rules",
                loaded.sidecar_id.0,
                loaded.config.bitcoin_nodes.len(),
                loaded.config.collectors.len(),
                loaded.config.notification_rules.len(),
            );
        }
        Err(e) => {
            eprintln!("config error: {}", e);
            std::process::exit(EX_CONFIG);
        }
    }
}
