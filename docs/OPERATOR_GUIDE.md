# Bithound Operator Guide (V0)

Bithound is an observability sidecar that runs next to a Bitcoin Core
node, polls it on a fixed interval, evaluates a small set of
diagnostic rules against the observed state, and notifies the
operator when a real incident lifts.

This guide is for someone running the V0 sidecar against a single
Bitcoin Core node. It covers what V0 actually does, how to install
and configure it, how to interpret the three shipped rules, and the
common ways operators get stuck.

## Table of contents

1. [What V0 does and doesn't do](#what-v0-does-and-doesnt-do)
2. [Install](#install)
3. [Writing `bithound.toml`](#writing-bithoundtoml)
4. [Bitcoin Core RPC setup](#bitcoin-core-rpc-setup)
5. [Notification sinks](#notification-sinks)
6. [The three V0 diagnostic rules](#the-three-v0-diagnostic-rules)
7. [Operator HTTP API](#operator-http-api)
8. [Where logs and the database live](#where-logs-and-the-database-live)
9. [Troubleshooting](#troubleshooting)

## What V0 does and doesn't do

**V0 does:**

- Polls one Bitcoin Core node on a configurable interval (typically
  10 seconds) over JSON-RPC. The poll runs four RPC calls in
  parallel: `getblockchaininfo`, `getmempoolinfo`, `getnetworkinfo`,
  `getpeerinfo`.
- Records every observation in a local SQLite database with a
  configurable retention window.
- Evaluates three diagnostic rules against the observed state and
  opens / clears incidents based on their fingerprints.
- Routes incident lifecycle events (`Opened`, `Escalated`, `Resolved`)
  to Telegram, Discord, or generic webhook sinks per operator-defined
  notification rules.
- Persists an audit row for every notification attempt so a delivery
  failure leaves a forensic trail.
- Serves a local read-only HTTP API on `127.0.0.1:8487` by default
  (`GET /health`, `GET /incidents/open`, `GET /incidents/:id`,
  `GET /incidents/:id/evidence`) so operators can inspect state with
  `curl | jq` without waiting for a push notification.
- Survives Bitcoin Core restarts, sidecar restarts, and SIGTERM —
  the sidecar's UUIDv7 identity is persisted in `id_file` and reused
  on every restart so observation provenance stays stable.

**V0 doesn't:**

- Monitor LND or Elements. The configuration schema accepts
  `[[lnd_nodes]]` and `[[hosts]]` blocks so V0.1 can ship without a
  config migration, but no collectors are wired for those targets.
- Subscribe to ZMQ. The `zmq_endpoint` field on `[[bitcoin_nodes]]`
  is parsed but not used; ZMQ subscription collectors land in V0.1.
- Run a browser UI. The operator API speaks JSON over loopback HTTP;
  pair it with `curl` and `jq`, or query the `db_path` SQLite file
  directly. A browser UI is V0.2.
- Implement suppression rules or maintenance windows. The
  `SuppressionRule` shape exists but the V0 notifier doesn't gate on
  it.
- Auto-update. Operators upgrade by replacing the binary.

## Install

V0 ships as a Rust binary. The only supported install path today is
`cargo install --path .` from a checkout of this repo.

```bash
git clone https://github.com/bithoundhq/bithound.git
cd bithound
cargo install --path . --locked
```

The resulting `bithound` binary needs write access to:

- the directory containing `[sidecar].id_file` (default
  `/var/lib/bithound/sidecar_id`)
- the directory containing `[storage].db_path` (default
  `/var/lib/bithound/bithound.db`)

A signed binary release and Docker image land alongside the V0
ship; until then, build from source.

## Writing `bithound.toml`

The config lives wherever you point `--config`. If `--config` is
omitted, the binary looks for `./bithound.toml`, then
`/etc/bithound/bithound.toml`. If neither exists the binary exits
with `EX_CONFIG=78` and a clear "where I looked" error.

A minimal V0 config:

```toml
[sidecar]
id_file = "/var/lib/bithound/sidecar_id"
log_level = "info"

[storage]
db_path = "/var/lib/bithound/bithound.db"

[[bitcoin_nodes]]
id = "alice"
rpc_url = "http://127.0.0.1:8332"

[bitcoin_nodes.auth]
type = "user_pass"
user = "bithound"
password_env = "BITHOUND_BITCOIN_ALICE_PASSWORD"

[[collectors]]
id = "alice-rpc"
target = { type = "bitcoin_node", id = "alice" }
integration = { type = "bitcoin_core_rpc", interval_seconds = 10 }
instance_label = "alice"

[notifications.telegram]
bot_token_env = "BITHOUND_TELEGRAM_BOT_TOKEN"
parse_mode = "html"

[[notification_rules]]
id = "critical-to-telegram"
name = "Critical incidents → Telegram"
enabled = true
min_severity = "critical"
event_kinds = []

[notification_rules.target]
type = "telegram"
chat_id = -1001234567890
```

Full example with every supported field is in
[`examples/bithound.example.toml`](../examples/bithound.example.toml).

**Inline-secret guard.** Bithound refuses to start if any field
named `*_password`, `*_token`, or `*_secret` lacks the mandatory
`_env` suffix. Every secret is referenced by env-var name; the
actual value is read at startup from the process environment. This
keeps tokens and passwords out of the file the operator commits to
git.

**Override via env.** Any top-level config key can be overridden
with `BITHOUND_<section>__<key>=...`. For example:

```bash
BITHOUND_RUNTIME__CHANNEL_CAPACITY=2048 bithound --config /etc/bithound/bithound.toml
```

Useful for one-off retention bumps or capacity tweaks without
editing the file.

## Bitcoin Core RPC setup

Bithound needs read-only RPC access to the four endpoints listed
above. Both auth schemes the V0 collector supports are documented
here.

### Option A — `user_pass`

In `bitcoin.conf`:

```
rpcauth=bithound:<salt>$<hash>
rpcallowip=127.0.0.1
rpcport=8332
```

Generate the salt/hash pair with
[`share/rpcauth/rpcauth.py`](https://github.com/bitcoin/bitcoin/blob/master/share/rpcauth/rpcauth.py)
from the Bitcoin Core repo. The password the script prints is the
value you set in `$BITHOUND_BITCOIN_<id>_PASSWORD` at startup —
**never** in the TOML.

In `bithound.toml`:

```toml
[bitcoin_nodes.auth]
type = "user_pass"
user = "bithound"
password_env = "BITHOUND_BITCOIN_ALICE_PASSWORD"
```

### Option B — `cookie_file`

If bithound runs on the same machine as bitcoind under the same
user, the cookie file is the simplest path. No secret to set:

```toml
[bitcoin_nodes.auth]
type = "cookie_file"
path = "/var/lib/bitcoind/.cookie"
```

Bithound reads the cookie on every poll, so a `bitcoin-cli stop`
followed by restart won't break the sidecar — the new cookie is
picked up automatically.

### Confirming the RPC reaches you

```bash
bitcoin-cli -rpcwait getblockchaininfo
```

If that returns JSON for your authenticated bitcoind user, bithound
can poll. The four targets bithound calls are documented as the
canonical health surface in
[`src/collectors/bitcoin_core/rpc.rs`](../src/collectors/bitcoin_core/rpc.rs).

## Notification sinks

V0 supports three sink types. Each `[[notification_rules]]` block
picks exactly one. A single bithound deployment can mix sinks —
e.g. Critical → Telegram, Warning → Discord, everything →
PagerDuty via webhook.

### Telegram

One bot token serves every Telegram rule; each rule picks its own
`chat_id`.

```toml
[notifications.telegram]
bot_token_env = "BITHOUND_TELEGRAM_BOT_TOKEN"
parse_mode = "html"  # or "plain"

[[notification_rules]]
id = "critical-to-tg"
name = "Critical incidents → Telegram"
enabled = true
min_severity = "critical"
event_kinds = []

[notification_rules.target]
type = "telegram"
chat_id = -1001234567890
```

The bot token comes from `@BotFather`. The `chat_id` for a group
chat is negative; you can read it from `getUpdates` after a test
message.

### Discord

Each Discord rule carries its own webhook URL (via env-var
reference) and optional thread id.

```toml
[[notification_rules]]
id = "warnings-to-discord-ops"
name = "Warnings → Discord #ops"
enabled = true
min_severity = "warning"
event_kinds = []

[notification_rules.target]
type = "discord"
webhook_env = "BITHOUND_OPS_DISCORD_WEBHOOK"
# thread_id = 1234567890123456789  # optional
```

The webhook URL is created in Discord's Server Settings → Integrations
→ Webhooks. Bithound never logs the URL itself.

### Generic webhook

For PagerDuty, Opsgenie, Slack incoming-webhooks, or your own
internal incident bus.

```toml
[[notification_rules]]
id = "critical-to-pagerduty"
name = "Critical incidents → PagerDuty"
enabled = true
min_severity = "critical"
event_kinds = []

[notification_rules.target]
type = "webhook"
url_env = "BITHOUND_OPS_PAGERDUTY_WEBHOOK"
```

Webhook POST body shape:

```json
{
  "event": "Opened",
  "incident_id": "01956c39-...",
  "kind": "bitcoin.tip_lag_or_ibd_stalled",
  "severity": "Critical",
  "title": "...",
  "summary": "...",
  "affected_component": "bitcoin_node/alice",
  "diagnostic_summary": "...",
  "occurred_at": "2026-05-22T12:34:56.789Z"
}
```

`event` is one of `Opened`, `Escalated`, `Resolved`.

### Per-rule filters

Every `[[notification_rules]]` block carries:

- `min_severity` — `info` / `warning` / `critical`. The rule fires
  only when the incident's current severity is **at or above** this
  threshold. V0 rules emit Critical, so practical values are
  `warning` (catches everything) or `critical` (catches everything).
- `event_kinds` — list of incident-kind names. Empty matches every
  kind. Use this when you want, say, only `bitcoin.no_peers` going
  to a specific channel.
- `enabled` — flip to `false` to suspend a rule without deleting it.

## The three V0 diagnostic rules

### `bitcoin.rpc_unreachable`

**Fires when** all four Bitcoin RPC health-check targets
(`getblockchaininfo`, `getmempoolinfo`, `getnetworkinfo`,
`getpeerinfo`) report `HealthStatus::Critical` for ≥ 60 seconds.

**Clears when** any one of them returns to `HealthStatus::Ok`.

**What to do.** First check whether bitcoind is up. If it is, check
the RPC port + auth — that's the most common cause of a sustained
all-four-Critical outage.

**Source:** [`src/diagnostics/rules/bitcoin/rpc_unreachable.rs`](../src/diagnostics/rules/bitcoin/rpc_unreachable.rs)

### `bitcoin.no_peers`

**Fires when** `getnetworkinfo.connections_out == 0` AND
`networkactive == true` continuously for ≥ 60 seconds.

**Clears when** the outbound peer count returns to non-zero.

**Silent when** the operator has deliberately disabled networking
(`networkactive == false`). Tightens incident-catalog entry
[A3](INCIDENT_CATALOG.md#a3-outbound-peer-starvation) from the
original "< 8 outbound" to the unambiguous zero case so the V0
alert is high-signal.

**What to do.** Check firewall and port 8333 reachability. The
node may be partitioned, the ISP may be blocking, or addrman may
have churned through every known peer. Add manual peers via
`bitcoin-cli addnode` to known-good nodes while you investigate.

**Source:** [`src/diagnostics/rules/bitcoin/no_peers.rs`](../src/diagnostics/rules/bitcoin/no_peers.rs)

### `bitcoin.tip_lag_or_ibd_stalled`

**Fires when** *either* pattern below holds across two consecutive
polls:

- **A1 (tip lag):** `initialblockdownload == true` AND
  `headers - blocks < 1000` AND `verificationprogress > 0.999`
  AND `peer_count ≥ 8`. The node thinks it's syncing but it's
  effectively at the tip.
- **A2 (IBD stall):** `headers - blocks ≥ 1000` AND
  `verification_progress` is flat (no change > 1e-9) across the
  last 5 minutes. The node is actually syncing but the download
  window has stalled.

**Clears when** neither pattern holds across two consecutive polls.

**What to do.** For the A1 shape, see
[A1 in the incident catalog](INCIDENT_CATALOG.md#a1-tip-lag--node-believes-it-is-in-ibd-when-it-shouldnt-be).
For A2, see
[A2 in the incident catalog](INCIDENT_CATALOG.md#a2-ibd-stall--block-download-window-starvation).
The fixes diverge — A1 is usually a `-maxtipage` restart or
`reconsiderblock`; A2 is usually a peer churn.

**Source:** [`src/diagnostics/rules/bitcoin/tip_lag_or_ibd_stalled.rs`](../src/diagnostics/rules/bitcoin/tip_lag_or_ibd_stalled.rs)

## Operator HTTP API

Bithound serves a local read-only HTTP API so you can ask "what is
broken right now?" without waiting for a notification or shelling
into `sqlite3`. By default it binds `127.0.0.1:8487`. The bind is
loopback-only and there is **no authentication, no CORS, no TLS** —
this is a V0 trade-off, not an oversight. Only local processes can
reach the API; if you need remote access, run `bithound` behind a
reverse proxy that adds those layers.

Configurable via the `[api]` block:

```toml
[api]
bind = "127.0.0.1:8487"   # optional, default 127.0.0.1:8487
enabled = true             # optional, default true; set false to skip
```

The four endpoints, all `GET`, all JSON:

| Endpoint | What it returns |
| --- | --- |
| `/health` | Sidecar liveness + DB reachability + uptime. 200 when reachable, 503 when DB unreachable (body shape identical either way). |
| `/incidents/open` | Every incident with `status != Resolved`, newest first. Empty array when none. |
| `/incidents/:id` | Full incident detail by UUID. 404 if unknown, 400 if `:id` is not a UUID. |
| `/incidents/:id/evidence` | Dereferences the incident's `evidence` array into the full underlying observations. Observations that retention has swept are silently omitted. |

Quick examples:

```bash
# Is the sidecar healthy?
curl -s localhost:8487/health | jq .

# What's broken right now?
curl -s localhost:8487/incidents/open | jq '.incidents[] | {kind, severity, opened_at, summary}'

# Drill into one incident
ID=$(curl -s localhost:8487/incidents/open | jq -r '.incidents[0].id')
curl -s localhost:8487/incidents/$ID | jq .
curl -s localhost:8487/incidents/$ID/evidence | jq '.evidence[] | {observation_id, observed_at, payload}'
```

The `[api].enabled = false` setting skips the API task entirely —
useful for embedded deployments and for tests that exercise the rest
of the runtime without binding a port.

## Where logs and the database live

**Logs.** Bithound emits structured tracing events to stderr at the
level set by `[sidecar].log_level` (or the `RUST_LOG` env var).
Sample at `info`:

```
2026-05-22T12:34:56.789Z INFO bithound::runtime bithound runtime starting sidecar_id=... polling_collectors=1 notification_rules=2 diagnostic_rules=3
```

Pipe it to your log shipper of choice (`journalctl`, `vector`,
`fluent-bit`). Bithound does no log rotation itself.

**Database.** `[storage].db_path` is the SQLite file. The schema is
under [`migrations/`](../migrations/) and applied on every startup
via `sqlx::migrate!`. Tables you can query directly:

- `observations` — every observation the sidecar persisted, with
  the originating collector + sidecar provenance.
- `incidents` — open + resolved incidents with fingerprint
  (`<subject_kind>|<subject_id>|<incident_kind>|<dimension or '-'>`)
  as the deduplication key.
- `notification_attempts` — one row per delivery attempt. Status is
  `Pending` until the worker terminates it; `Delivered`, `Failed`,
  or `Rejected` after.

Retention is configurable via `[storage.retention]` (defaults: 30
days observations, 365 days incidents). A background task vacuums
on the configured interval.

## Troubleshooting

### Exit code 78 (`EX_CONFIG`) on startup

The config layer rejected your `bithound.toml`. Bithound exits 78
on every config failure, with a specific error message on stderr.
Common shapes:

- **`config error: NotFound`** — no config at `--config`, no
  `./bithound.toml`, no `/etc/bithound/bithound.toml`. Pass
  `--config <path>` or put the file in one of the default
  locations.
- **`config error: ... is missing`** — required env var (e.g.
  `BITHOUND_BITCOIN_ALICE_PASSWORD`) wasn't set. Either set it or
  remove the rule that references it.
- **`config error: ... has inline secret`** — you wrote a literal
  password / token / secret in the TOML. Move the value to an env
  var and reference it via `*_env`.
- **`config error: unknown field "foo"`** — typo in a key name.
  `deny_unknown_fields` is on across every config section so the
  loader will tell you exactly where to look.
- **`config error: collector ... targets unknown id "..."`** — the
  collector's `target.id` doesn't match any `[[bitcoin_nodes]].id`.

### Bithound runs but no incidents fire

1. Check the sidecar is actually polling. Set `RUST_LOG=debug` and
   look for `polling collector loaded` and per-poll events on
   startup.
2. Check the RPC is reachable. `bitcoin-cli getblockchaininfo`
   should succeed with the same credentials.
3. Check the notification rule's `min_severity` and `event_kinds`.
   V0 rules emit Critical, so `min_severity = "critical"` works;
   `min_severity = "warning"` works; `min_severity = "info"` works;
   but `event_kinds = ["bitcoin.no_peers"]` won't match a
   `bitcoin.rpc_unreachable` incident.
4. Check the rule's debounce. `bitcoin.rpc_unreachable` and
   `bitcoin.no_peers` need 60 seconds of continuous condition;
   `bitcoin.tip_lag_or_ibd_stalled` needs two consecutive polls
   (so ~2 × `interval_seconds`).

### A notification was sent but never arrived

Inspect the `notification_attempts` table:

```bash
sqlite3 /var/lib/bithound/bithound.db \
  "SELECT id, lifecycle_kind, target_kind, status, outcome FROM notification_attempts ORDER BY rowid DESC LIMIT 10"
```

`status = Pending` means the dispatch worker accepted the row but
hasn't terminated it — usually a still-in-flight HTTP request, or
the worker died mid-dispatch and the row is now an orphan audit
record (V0 doesn't auto-retry these; V0.1's retry scheduler lands
in BTH-53).

`status = Failed` carries the failure reason in `outcome`. Most
common: webhook URL 4xx (revoked Discord webhook, bad PagerDuty
token), or DNS lookup failure on a long-deployed config.

### "Bithound is restarting forever"

Per ADR-S2, the supervisor exponential-backs-off on collector
panic / `Err`: 10s → 30s → 60s → 300s, resetting after a clean
5-minute run. If the collector's underlying RPC is fundamentally
broken (URL unreachable, auth invalid, bitcoind off entirely),
bithound will spin at the long backoff but not exit. Check the
logs for the underlying error.

### "How do I prove the smoke test still passes after my change?"

```bash
cargo test --ignored --test e2e_tip_lag
```

That's the V0 end-to-end test (BTH-40). It spins a mock bitcoind
returning the A1 firing pattern and a mock webhook receiver, then
asserts the webhook gets an `Opened bitcoin.tip_lag_or_ibd_stalled`
event within 30 seconds. See [`tests/README.md`](../tests/README.md)
for the contract.
