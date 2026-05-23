# Configuration schema

The canonical shape of `bithound.toml`. Every section uses
`deny_unknown_fields` — a typo names itself at load time. The
operator-facing tour with rationale lives at
[Operator guide → Configuration](../operator/configuration.md); the
linear walkthrough with sink credentials lives at
[`docs/OPERATOR_GUIDE.md`](https://github.com/bithoundhq/bithound/blob/main/docs/OPERATOR_GUIDE.md).

A copyable annotated example with every supported field is
[`examples/bithound.example.toml`](https://github.com/bithoundhq/bithound/blob/main/examples/bithound.example.toml).

## Top-level sections

| Section | Required | Loader type |
| --- | --- | --- |
| `[sidecar]` | yes | `SidecarConfig` |
| `[storage]` | yes | `StorageConfig` |
| `[runtime]` | no (defaults) | `RuntimeConfig` |
| `[api]` | no (defaults) | `ApiConfig` |
| `[incidents]` | no | `IncidentsConfig` |
| `[[bitcoin_nodes]]` | no | `Vec<BitcoinNodeConfig>` |
| `[[lnd_nodes]]` | no (reserved) | `Vec<LndNodeConfig>` |
| `[[hosts]]` | no (reserved) | `Vec<HostConfig>` |
| `[[collectors]]` | yes (≥1) | `Vec<CollectorDescriptorConfig>` |
| `[notifications]` | no | `NotificationsConfig` |
| `[[notification_rules]]` | no | `Vec<NotificationRuleConfig>` |

## `[sidecar]`

```toml
[sidecar]
id_file = "/var/lib/bithound/sidecar_id"   # required, path
log_level = "info"                          # optional, default "info"
```

`log_level` is whatever `tracing_subscriber::EnvFilter` accepts
(e.g. `"info"`, `"bithound=debug,sqlx=warn"`).

## `[storage]`

```toml
[storage]
db_path = "/var/lib/bithound/bithound.db"   # required, path

[storage.retention]                          # all fields optional
observations_max_age_days = 30
incidents_max_age_days    = 365
suppressions_grace_days   = 90
vacuum_interval_hours     = 24
```

The `sqlx` SQLite pool is opened against `db_path` at startup; the
parent directory must be writable.

## `[runtime]`

```toml
[runtime]
channel_capacity = 1024                # optional, default 1024
shutdown_deadline_seconds = 30         # optional, default 30
```

`channel_capacity` is the bounded `mpsc::channel<ObservationBatch>`
between collectors and the consumer task. `shutdown_deadline_seconds`
is the maximum time the supervisor waits for in-flight work on a
SIGTERM before force-aborting.

## `[api]`

```toml
[api]
bind = "127.0.0.1:8487"     # optional, default 127.0.0.1:8487
enabled = true               # optional, default true
```

Local read-only HTTP API for operator queries. `bind` accepts any
valid `SocketAddr` string; the default is loopback-only because V0
ships with no authentication, no CORS, and no TLS. Setting
`enabled = false` skips the API task entirely. The four V0 endpoints
(`GET /health`, `GET /incidents/open`, `GET /incidents/:id`,
`GET /incidents/:id/evidence`) are described in
[Operator guide → HTTP API](../operator/configuration.md#operator-http-api).

## `[incidents]`

```toml
[incidents]
kinds_config_path = "/etc/bithound/custom_kinds.toml"  # optional
```

If set, the catalog at that path is loaded **additively** on top of
the built-in V0 catalog. See
[Custom incidents](../operator/custom-incidents.md) and the
[incident-kind schema](incident-kinds.md).

## `[[bitcoin_nodes]]`

```toml
[[bitcoin_nodes]]
id = "alice"                              # required, unique per file
rpc_url = "http://127.0.0.1:8332"         # required
zmq_endpoint = "tcp://127.0.0.1:28332"    # optional, V0.1 only

[bitcoin_nodes.auth]
# Variant A — user_pass (remote bitcoind, shared user)
type = "user_pass"
user = "bithound"
password_env = "BITHOUND_BITCOIN_ALICE_PASSWORD"

# Variant B — cookie_file (same host, same user as bitcoind)
# type = "cookie_file"
# path = "/var/lib/bitcoind/.cookie"
```

`id` is the slug `[[collectors]].target.id` and notification
attributions reference. `zmq_endpoint` is parsed for forward
compatibility; V0 has no ZMQ collector.

## `[[collectors]]`

```toml
[[collectors]]
id = "alice-rpc"                                                       # required
target = { type = "bitcoin_node", id = "alice" }                       # required
integration = { type = "bitcoin_core_rpc", interval_seconds = 10 }     # required
instance_label = "alice"                                               # required
description = "Bitcoin Core RPC polling for alice"                     # optional
```

V0 supports one integration kind:

- **`bitcoin_core_rpc`** with `interval_seconds: u32` — the polling
  cadence in seconds.

Other variants are parsed and rejected at runtime build time:

- `bitcoin_core_zmq`, `lnd_grpc_stream` — subscription kinds, V0.1+.
- `lnd_grpc_poll`, `lnd_rest`, `host` — polling kinds, V0.1+.

Cross-reference: a collector's `target.id` must match an entry in
the corresponding `[[bitcoin_nodes]]` / `[[lnd_nodes]]` / `[[hosts]]`
section. Otherwise the loader fails with
`collector ... targets unknown id ...`.

## `[notifications]`

Sink-wide settings. Per-rule target details live on
`[[notification_rules]]`.

```toml
[notifications.telegram]                       # optional
bot_token_env = "BITHOUND_TELEGRAM_BOT_TOKEN"   # required if section present
parse_mode = "html"                             # optional, "html" | "plain" | "markdown_v2"

[notifications.discord]                         # optional (V0 has no sink-wide fields)

[notifications.webhook]                         # optional (V0 has no sink-wide fields)
```

The Discord and Webhook sink-wide sections are present so V0.1+ can
add fields without a config migration.

## `[[notification_rules]]`

```toml
[[notification_rules]]
id = "critical-to-telegram"            # required, operator-picked slug
name = "Critical incidents → Telegram"  # required, free-form
enabled = true                          # required
min_severity = "critical"               # required: "info" | "warning" | "critical"
event_kinds = []                        # optional, default []. Empty = match every kind.

[notification_rules.target]
# Variant A
type = "telegram"
chat_id = -1001234567890                # i64, negative for groups

# Variant B
# type = "discord"
# webhook_env = "BITHOUND_OPS_DISCORD_WEBHOOK"
# thread_id = 1234567890123456789       # optional, u64

# Variant C
# type = "webhook"
# url_env = "BITHOUND_OPS_PAGERDUTY_WEBHOOK"
```

`event_kinds` filters by incident-kind name. With `event_kinds = []`
every kind matches; with `event_kinds = ["bitcoin.no_peers"]` only
`bitcoin.no_peers` matches.

## Inline-secret guard

Any field whose name ends in `_password`, `_token`, or `_secret`
(case-insensitive) is rejected if it contains a literal value. Every
secret is referenced by env-var name; the actual value is read at
startup from the process environment via the `*_env` field.

Concretely:

- `[bitcoin_nodes.auth] type = "user_pass"` requires `password_env`,
  rejects `password`.
- `[notifications.telegram]` requires `bot_token_env`, rejects
  `bot_token`.
- `[notification_rules.target] type = "discord"` requires
  `webhook_env`, rejects `webhook_url` (URL is the credential).
- `[notification_rules.target] type = "webhook"` requires `url_env`,
  rejects `url`.

The loader names the offending dotted path in the error message.

## Env-var override syntax

Any top-level non-secret key can be overridden with
`BITHOUND_<section>__<key>=...`. The double-underscore (`__`)
separates the section name from the key name. Type coercion happens
against the existing TOML value: an integer stays integer, a bool
stays bool.

Examples:

```bash
BITHOUND_RUNTIME__CHANNEL_CAPACITY=2048 \
  bithound --config /etc/bithound/bithound.toml

BITHOUND_SIDECAR__LOG_LEVEL=debug \
  bithound --config /etc/bithound/bithound.toml
```

Secrets (`*_env` references) come from the env regardless of this
override system — the override mechanism is for non-secret tuning.
