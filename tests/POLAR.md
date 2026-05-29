# Polar regtest harness for the LND e2e tests

`e2e_lnd_b1_via_polar.rs` is `#[ignore]`-gated because it needs a
running [Polar](https://lightningpolar.com/) network — a docker-based
test stack that spins up real bitcoind + real LND nodes. CI doesn't
have Docker, so the test only runs when an operator sets the env
vars below by hand.

## One-time setup

1. Install Polar (brew on macOS, AppImage on Linux, MSI on Windows).
   It's a desktop app that drives a docker-compose stack under the
   hood; if you already have docker running, Polar just wires it up.
2. Open Polar, click **Create Lightning Network**, and accept the
   defaults: a single `bitcoind` node + two `LND` nodes wired
   together. Polar names them `alice`, `bob`, `carol` by default.
3. Click **Start** and wait for every node to report green.
4. Open a channel between `alice` and `bob`:
   - Click `alice` → **Actions** tab → **Open Outgoing Channel**.
   - Pick `bob` as the peer, amount `1 000 000` sat, accept.
   - Polar mines blocks until the channel confirms.

## Extracting the LND artifacts bithound needs

Polar persists each LND node's files under
`~/.polar/networks/<network-id>/volumes/lnd/<node>/`. For node
`alice`:

| Artifact | Path inside the LND volume |
| --- | --- |
| TLS cert | `tls.cert` |
| Read-only macaroon | `data/chain/bitcoin/regtest/readonly.macaroon` |
| gRPC port | shown in Polar's `alice` → **Info** tab (typically `10001` for the first LND) |

Polar binds each LND's gRPC port to `127.0.0.1:<polar-assigned-port>`
on the host. The Info tab shows the resolved port; copy it.

## Env vars the e2e test reads

```bash
export BITHOUND_TEST_POLAR_LND_GRPC="https://127.0.0.1:10001"
export BITHOUND_TEST_POLAR_LND_CERT="$HOME/.polar/networks/1/volumes/lnd/alice/tls.cert"
export BITHOUND_TEST_POLAR_LND_MACAROON_HEX=$(xxd -ps -u -c 1000 \
  "$HOME/.polar/networks/1/volumes/lnd/alice/data/chain/bitcoin/regtest/readonly.macaroon")
export BITHOUND_TEST_POLAR_BITCOIN_RPC="http://127.0.0.1:18443"
export BITHOUND_TEST_POLAR_BITCOIN_USER="polaruser"
export BITHOUND_TEST_POLAR_BITCOIN_PASS="polarpass"
```

`xxd -ps -u -c 1000` flattens the macaroon bytes to a single hex line
(no newlines, no spaces); bithound parses that shape into a
`SecretString`.

## Running the test

```bash
cargo test --test e2e_lnd_b1_via_polar -- --ignored --nocapture
```

The `--nocapture` is worth keeping the first time — bithound's
startup tracing lands on stderr and confirms the LND collector
connected before the test starts probing the channel state.

## What the B1 test exercises

The test drives the catalog's **B1 — Channel inactive — peer offline**
pattern:

1. Spawn bithound against the Polar config above with a mock webhook
   listening on a free port.
2. Wait for the first poll to produce a green `lnd.channel_detail`
   observation for the alice↔bob channel.
3. Pause `bob` in Polar (right-click → **Stop**). Wait the channel's
   private/public debounce window (default 5 min public).
4. Assert the webhook received an `lnd.channel_inactive` Active event
   targeting the alice↔bob channel with severity Warning (peer
   offline) or Critical (peer online — should not happen for this
   scenario, but the test asserts the actual severity for the
   record).
5. Resume `bob`. Assert the rule emits Cleared within one debounce
   window after the channel reactivates.

## What the B6 test exercises (planned)

`e2e_lnd_b6_via_polar.rs` is a separate scaffold (not yet written)
covering **B6 — Watchtower / chain backend lag**: pause Polar's
bitcoind, wait for LND's `block_height` to diverge by more than 2
blocks for 60 s, assert `lnd.chain_backend_lag` Active fires. Then
resume bitcoind, wait for the height to converge, assert Cleared.
