# Integration tests

This directory holds end-to-end tests that exercise the full `bithound`
pipeline against mock external services. They are slower and noisier
than the inline `#[cfg(test)]` unit tests under `src/`, so they're
`#[ignore]`-gated by default — `cargo test` in CI does not spin them
up on every run.

## Running locally

Default `cargo test` (unit + crate-level integration, no ignored tests):

```bash
cargo test
```

Run every ignored e2e test (currently one):

```bash
cargo test --ignored
```

Run only the V0 tip-lag smoke test:

```bash
cargo test --test e2e_tip_lag -- --ignored --nocapture
```

The `BITHOUND_TEST_REGTEST=1 cargo test` alias also runs the e2e
suite — it sets the env-var-gated test path that future
`tests/regtest_*.rs` files (against a real `bitcoind` regtest node)
will read.

## What e2e_tip_lag covers

`tests/e2e_tip_lag.rs` is the V0 smoke test (BTH-40). It:

1. Spawns a hand-rolled JSON-RPC server on an ephemeral port that
   impersonates `bitcoind`. Every poll returns the A1 firing pattern:
   `initialblockdownload = true`, `blocks = 899_500`, `headers = 900_000`
   (gap = 500, below the rule's 1000 ceiling), `verificationprogress
   = 0.99996`, and ten peer entries.
2. Spawns a second HTTP server that captures every POST body into a
   shared `Vec<serde_json::Value>` — the mock webhook receiver.
3. Writes a temp `bithound.toml` pointing the `bitcoin_core_rpc`
   collector at the mock bitcoind and one `notification_rules`
   webhook target at the mock receiver. Sidecar id and SQLite db
   live in the same tempdir, so the run is fully self-contained.
4. Spawns the `bithound` binary as a child process via
   `env!("CARGO_BIN_EXE_bithound")` with `--config`.
5. Polls the captured webhook bodies for up to 30 seconds, waiting
   for an `Opened` lifecycle event whose `kind ==
   "bitcoin.tip_lag_or_ibd_stalled"`.
6. Asserts the payload fields (`event`, `kind`, `severity`,
   `incident_id`, `affected_component`) match expectations.

If the test times out, the panic message includes every captured
POST body plus the child's stdout and stderr, so a flaky CI run
can be debugged from the failure log alone.

## Prerequisites

* The `bithound` binary must be buildable — the test resolves it
  via `env!("CARGO_BIN_EXE_bithound")`, which cargo computes from
  the workspace's primary `[[bin]]`.
* No external services. The mock bitcoind RPC and mock webhook are
  hand-rolled HTTP servers bound to `127.0.0.1:0`.
* `tempfile` is already a dev-dependency.

## Adding new e2e tests

* Put each scenario in its own `tests/e2e_*.rs` file. Cargo
  compiles each top-level file as a separate integration-test
  binary, so a slow scenario doesn't drag the rest along.
* Mark every e2e test `#[ignore = "..."]` with a one-line
  rationale and an opt-in command in the message.
* Prefer mock external services over real ones. The current
  e2e_tip_lag test demonstrates the pattern; lift its
  `spawn_mock_bitcoind` / `spawn_mock_webhook` shape rather than
  spinning up the real `bitcoind` for every test.
* A real-regtest variant (real `bitcoind -regtest`) is the next
  step up. Land it under `tests/regtest_*.rs`, gate it on
  `BITHOUND_TEST_REGTEST=1`, and document the bitcoind setup
  prerequisite here.
