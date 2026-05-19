# Configuration

> **Stub.** The configuration loader has not landed yet.

When it does, Bithound will read a TOML file (path supplied via the CLI
or a well-known default) with sections for:

- **Subjects** — which Bitcoin nodes, LND nodes, and hosts to observe,
  and how to reach each one.
- **Collectors** — polling intervals, subscription endpoints, retry
  policy.
- **Storage** — the SQLite database path.
- **Notifications** — Telegram bot token, Discord webhook, generic
  webhook targets; per-target severity filters and suppression rules.
- **Incident-kind catalog** — an optional pointer to a user-supplied
  TOML file that adds operator-defined incident kinds on top of the
  built-in catalog. See [Custom incidents](custom-incidents.md).

The full schema will be documented under
[Reference → Configuration schema](../reference/config-schema.md).
