# Installation

V0 ships as a single Rust binary. The supported install path today is
`cargo install` from a checkout of the repository. A signed binary
release and a Docker image land alongside the V0 ship.

## Prerequisites

- A stable Rust toolchain (`rustup default stable`).
- A reachable Bitcoin Core JSON-RPC endpoint. Bithound uses
  `getblockchaininfo`, `getmempoolinfo`, `getnetworkinfo`, and
  `getpeerinfo` — every account that can call those four endpoints
  works.
- A writable filesystem path for the sidecar ID and the SQLite
  database (defaults: `/var/lib/bithound/sidecar_id` and
  `/var/lib/bithound/bithound.db`).

## Build from source

```bash
git clone https://github.com/bithoundhq/bithound.git
cd bithound
cargo install --path . --locked
```

`cargo install` drops the binary at `~/.cargo/bin/bithound`. Add it
to your `PATH` or copy it into `/usr/local/bin/`.

## File-system layout

Bithound needs write access to two locations the operator chooses
via the config file:

| Path | Default | Purpose |
| --- | --- | --- |
| `[sidecar].id_file` | `/var/lib/bithound/sidecar_id` | Persistent UUIDv7 — reused on every restart so observation provenance stays stable. Generated on first run. |
| `[storage].db_path` | `/var/lib/bithound/bithound.db` | SQLite database for observations, incidents, and notification attempts. |

Make both parent directories writable by the user bithound runs as.
A typical first-run setup:

```bash
sudo useradd --system --home /var/lib/bithound --shell /usr/sbin/nologin bithound
sudo install -d -o bithound -g bithound /var/lib/bithound /etc/bithound
```

## systemd unit (sketch)

```ini
[Unit]
Description=Bithound observability sidecar
After=network-online.target bitcoind.service
Wants=network-online.target

[Service]
Type=simple
User=bithound
Group=bithound
EnvironmentFile=/etc/bithound/bithound.env
ExecStart=/usr/local/bin/bithound --config /etc/bithound/bithound.toml
Restart=on-failure
RestartSec=10s

[Install]
WantedBy=multi-user.target
```

`/etc/bithound/bithound.env` is where you set the `BITHOUND_*_PASSWORD`
and `BITHOUND_*_WEBHOOK_URL` env vars referenced by your config —
keep it `0600`-readable by the bithound user only. The config TOML
itself never contains secret values; every secret is referenced by
env-var name (the field suffix ends in `_env`).

## Adding LND monitoring (v0.0.8.0+)

LND monitoring needs three artifacts from the LND host: a TLS cert,
a macaroon, and a reachable gRPC endpoint. Standard LND installs
keep these under `~/.lnd/`:

| Artifact | Default path | Purpose |
| --- | --- | --- |
| TLS cert | `~/.lnd/tls.cert` | LND's self-signed cert; bithound trusts ONLY this cert (no public CAs). |
| Macaroon | `~/.lnd/data/chain/bitcoin/<network>/readonly.macaroon` | Bytes carry the auth grant; read-only is sufficient for v0.0.8.0. |
| gRPC port | `:10009` (default) | Same host as LND; expose it as `https://...:10009`. |

Copy or mount the TLS cert into a path the bithound user can read
(e.g. `/var/lib/bithound/lnd.tls.cert`, `0644`). Read the macaroon
into an env var the systemd unit can hand to bithound:

```bash
sudo install -m 0644 -o bithound -g bithound \
  /path/to/lnd/tls.cert /var/lib/bithound/lnd.tls.cert

# In /etc/bithound/bithound.env — never commit this file:
BITHOUND_LND_ALICE_MACAROON=$(xxd -ps -u -c 1000 /path/to/readonly.macaroon)
```

Then add the LND blocks to `/etc/bithound/bithound.toml`:

```toml
[[lnd_nodes]]
id = "lnd-alice"
grpc_endpoint = "https://127.0.0.1:10009"
macaroon_env = "BITHOUND_LND_ALICE_MACAROON"
tls_cert_path = "/var/lib/bithound/lnd.tls.cert"
# Omit chain_backend_target_bitcoind_id when exactly one [[bitcoin_nodes]]
# is configured — the runtime resolves it automatically.

[[collectors]]
id = "lnd-alice-grpc"
target = { type = "lnd_node", id = "lnd-alice" }
integration = { type = "lnd_grpc_poll", interval_seconds = 10 }
instance_label = "alice"
description = "LND gRPC polling for alice"
```

bithound's [config-schema reference](../reference/config-schema.md#lnd_nodes)
documents every field. On `--check-config`, a missing TLS cert or a
gRPC endpoint without `https://` exits with `EX_CONFIG=78` and names
the offending field. **End-to-end verification follows BTH-67's
Polar regtest harness — see `tests/POLAR.md`.**

## Verifying

```bash
bithound --version
bithound --check-config --config /etc/bithound/bithound.toml
```

`--check-config` parses the file, validates the schema, and resolves
every `*_env` reference without starting the runtime. A clean exit
means the config is loadable; any failure exits 78 (`EX_CONFIG`)
with a structured error on stderr. See
[Troubleshooting in OPERATOR_GUIDE.md](https://github.com/bithoundhq/bithound/blob/main/docs/OPERATOR_GUIDE.md#troubleshooting)
for the common `EX_CONFIG` sub-cases.

## Upgrade and rollback

Bithound has no auto-update path. To upgrade:

1. Fetch the new source: `git pull`.
2. Rebuild: `cargo install --path . --locked --force`.
3. Restart the unit: `sudo systemctl restart bithound`.

To roll back, check out the previous tag and rebuild. The SQLite
schema is `sqlx`-migrated forward-only; downgrading the binary past
a schema migration requires manual DB surgery and is not supported
in V0.
