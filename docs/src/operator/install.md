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
