# Configuration

Bithound reads a single TOML file. Path resolution, in order:

1. `--config <path>` on the command line.
2. `./bithound.toml` in the working directory.
3. `/etc/bithound/bithound.toml`.

If none of those exist, the binary exits 78 (`EX_CONFIG`) with
`config error: NotFound`.

The full reference schema lives under
[Reference → Configuration schema](../reference/config-schema.md).
This page covers the operator-facing decisions and the inline-secret
rules every config must follow.

## Minimal config

This is everything needed to run V0 against a local Bitcoin Core node
authenticated by cookie file, with one webhook notification rule:

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
type = "cookie_file"
path = "/var/lib/bitcoind/.cookie"

[[collectors]]
id = "alice-rpc"
target = { type = "bitcoin_node", id = "alice" }
integration = { type = "bitcoin_core_rpc", interval_seconds = 10 }
instance_label = "alice"

[[notification_rules]]
id = "critical-to-webhook"
name = "Critical incidents → webhook"
enabled = true
min_severity = "critical"
event_kinds = []

[notification_rules.target]
type = "webhook"
url_env = "BITHOUND_OPS_WEBHOOK"
```

A fully annotated example with every supported field lives at
[`examples/bithound.example.toml`](https://github.com/bithoundhq/bithound/blob/main/examples/bithound.example.toml).

## Inline-secret guard

Bithound refuses to start if any field whose name ends in `_password`,
`_token`, or `_secret` (or matches those bare words) holds a literal
value. **Every secret is referenced by env-var name; the actual
value is read at startup from the process environment.**

The fields that follow this rule:

| Config field | Env-var field |
| --- | --- |
| `[bitcoin_nodes.auth] type = "user_pass"` | `password_env = "..."` |
| `[notifications.telegram]` | `bot_token_env = "..."` |
| `[notification_rules.target] type = "discord"` | `webhook_env = "..."` |
| `[notification_rules.target] type = "webhook"` | `url_env = "..."` |

A typo like `password = "..."` triggers an `inline secret` error at
load time with the dotted path of the offending key.

## Env-var overrides

Any top-level non-secret key can be overridden with
`BITHOUND_<section>__<key>=...`. Examples:

```bash
# Bump the consumer channel capacity for a high-throughput node.
BITHOUND_RUNTIME__CHANNEL_CAPACITY=2048 bithound --config /etc/bithound/bithound.toml

# Crank logging up to debug without editing the file.
BITHOUND_SIDECAR__LOG_LEVEL=debug bithound --config /etc/bithound/bithound.toml
```

The double-underscore (`__`) separates the section from the key.
Coercion happens against the TOML's declared type: an integer stays
integer, a bool stays bool.

## Config sections

| Section | Required | Purpose |
| --- | --- | --- |
| `[sidecar]` | yes | Sidecar identity file + log level filter. |
| `[storage]` | yes | SQLite database path + retention windows. |
| `[runtime]` | no | Channel capacity, shutdown deadline. Defaults are V0-tuned. |
| `[api]` | no | Operator HTTP API bind + enable flag. Defaults to loopback. |
| `[incidents]` | no | Optional path to a user-supplied incident-kind catalog. |
| `[[bitcoin_nodes]]` | no | One or more Bitcoin Core nodes to monitor. |
| `[[lnd_nodes]]` | no | One or more LND nodes to monitor (v0.0.8.0+). |
| `[[hosts]]` | no | Reserved for V0.9+; parsed but ignored. |
| `[[collectors]]` | yes (≥1) | Per-collector binding of an integration kind to a target. |
| `[notifications.telegram]` | no | Sink-wide Telegram config (one bot token serves every Telegram rule). |
| `[notifications.discord]` | no | Sink-wide Discord placeholder. |
| `[notifications.webhook]` | no | Sink-wide webhook placeholder. |
| `[[notification_rules]]` | no | One entry per (severity, kind-filter, target) triple. |

Every section uses `deny_unknown_fields`, so the loader names a typo
before it ships. See
[Reference → Configuration schema](../reference/config-schema.md) for
the per-field types and defaults.

## Bitcoin Core auth

Two auth schemes are supported:

- **`user_pass`** — for remote bitcoind or shared-user setups. Pair
  with `password_env`; generate the credentials with
  [`share/rpcauth/rpcauth.py`](https://github.com/bitcoin/bitcoin/blob/master/share/rpcauth/rpcauth.py)
  from the Bitcoin Core repo and put the resulting `rpcauth=` line
  into `bitcoin.conf`.
- **`cookie_file`** — for same-host same-user setups. Bithound reads
  the cookie on every poll, so `bitcoin-cli stop && restart` doesn't
  break the sidecar.

Full walkthroughs in the
[Operator guide](https://github.com/bithoundhq/bithound/blob/main/docs/OPERATOR_GUIDE.md#bitcoin-core-rpc-setup).

## Operator HTTP API

Bithound serves a local read-only HTTP API so you can inspect state
without waiting for a notification. By default it binds
`127.0.0.1:8487`. V0 ships with **no authentication, no CORS, no
TLS** — the loopback bind is the safety mechanism.

```toml
[api]
bind = "127.0.0.1:8487"   # optional, default 127.0.0.1:8487
enabled = true             # optional, default true
```

The four endpoints, all `GET`, all JSON:

- `/health` — sidecar liveness + DB reachability + uptime. 200 when
  reachable, 503 when the DB probe fails. Same body shape either way.
- `/incidents/open` — every incident with `status != Resolved`,
  newest first. Empty array when none.
- `/incidents/:id` — full incident detail by UUID. 404 on unknown id,
  400 on a non-UUID path.
- `/incidents/:id/evidence` — dereferences the incident's evidence
  array into the full underlying observations. Observations swept by
  retention are silently omitted.

Quick taste:

```bash
curl -s localhost:8487/health | jq .
curl -s localhost:8487/incidents/open | jq '.incidents[] | {kind, severity, summary}'
```

If you need remote access, run bithound behind a reverse proxy that
adds auth + TLS. `[api].enabled = false` disables the API task
entirely. The endpoint schemas are detailed in
[Operator guide → Operator HTTP API](https://github.com/bithoundhq/bithound/blob/main/docs/OPERATOR_GUIDE.md#operator-http-api).

## Custom incidents

To register additional incident kinds beyond the built-in V0 three,
point `[incidents].kinds_config_path` at a TOML file in the same
shape as
[`config/default_kinds.toml`](https://github.com/bithoundhq/bithound/blob/main/config/default_kinds.toml).
The user catalog is **additive**: it cannot override built-ins. See
[Custom incidents](custom-incidents.md) for the walkthrough and
[Reference → Incident-kind schema](../reference/incident-kinds.md)
for field-by-field details.
