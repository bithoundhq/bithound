# Configuration schema

> **Stub.** The configuration loader has not landed yet. The shape
> below is a sketch of what's coming.

## Top-level layout

```toml
# Path to a user-supplied incident-kind catalog (additive on top of
# the built-in catalog). Optional.
kinds_config_path = "/etc/bithound/custom_kinds.toml"

[storage]
database_path = "/var/lib/bithound/bithound.db"

[subjects.bitcoin_nodes.alice]
rpc_url   = "http://127.0.0.1:8332"
rpc_user  = "alice"
rpc_password_file = "/etc/bithound/secrets/bitcoind-rpc.pass"

[subjects.lnd_nodes.alice]
host       = "127.0.0.1:10009"
macaroon   = "/etc/bithound/secrets/admin.macaroon"
tls_cert   = "/etc/bithound/secrets/tls.cert"

[collectors.bitcoin_blockchain]
interval = "30s"

[notifications.telegram]
bot_token_file = "/etc/bithound/secrets/telegram.token"

[[notifications.telegram.subscriptions]]
chat_id = 123456789
min_severity = "warning"

[[notifications.webhook]]
url = "https://hooks.example.com/bithound"
hmac_secret_file = "/etc/bithound/secrets/webhook-hmac"
```

The fully specified schema, with required vs. optional markers and
validation rules, will land here when the loader does.
