# Notifications

Bithound routes incident lifecycle events — `Opened`, `Escalated`,
`Resolved` — to typed notification sinks. V0 ships three sink types:
Telegram, Discord, and generic webhook. A single deployment can mix
them: Critical to Telegram, Warning to Discord, everything to a
PagerDuty webhook, etc.

Every dispatch persists an audit row to `notification_attempts`
regardless of outcome. A delivery failure leaves a forensic trail you
can query later.

## Sink config shape

Each `[[notification_rules]]` block picks exactly one target type.
The rule's `min_severity` (`info` / `warning` / `critical`) and
`event_kinds` (empty matches every kind) filter which lifecycle
events fire on it.

### Telegram

One bot token serves every Telegram rule; each rule picks its own
`chat_id`. Get the bot token from `@BotFather`; for a group chat,
the `chat_id` is negative and readable from the bot's `getUpdates`
output after a test message.

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

`parse_mode = "html"` enables Bithound's HTML-escape formatter;
`parse_mode = "plain"` ships plain text. `markdown_v2` is mapped to
`html` internally so the operator's "formatted, not plain" intent is
preserved.

### Discord

Each Discord rule carries its own incoming webhook URL (via env var
reference) and an optional `thread_id` to post into a thread instead
of the channel root. Create the webhook in Discord's Server
Settings → Integrations → Webhooks.

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

Bithound never logs the webhook URL itself.

### Generic webhook

For PagerDuty, Opsgenie, Slack incoming-webhooks, internal incident
buses, or anything that accepts a JSON POST.

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

The POST body shape:

```json
{
  "title": "OPENED [Critical] bitcoin.tip_lag_or_ibd_stalled",
  "summary": "incident IncidentId(...) on BitcoinNode(BitcoinNodeId(\"alice\"))",
  "affected_component": null,
  "diagnostic_summary": null,
  "occurred_at": "2026-05-22T12:34:56.789Z"
}
```

The lifecycle kind (`OPENED` / `ESCALATED` / `RESOLVED`), the
incident severity, and the incident kind name are all embedded in the
`title` string in `"<KIND> [<Severity>] <incident_kind>"` shape — your
webhook receiver can pattern-match on the prefix.

## Per-rule filters

Every rule carries three knobs:

- **`min_severity`** — `info` / `warning` / `critical`. The rule fires
  only when the incident's current severity is at or above this
  threshold. V0 rules emit Critical, so `warning` and `critical` both
  match V0 traffic; `info` reserves room for V0.1 rules.
- **`event_kinds`** — list of incident-kind names. Empty matches every
  kind. Use this to route, say, `bitcoin.no_peers` to a specific
  channel.
- **`enabled`** — flip to `false` to suspend a rule without deleting
  it.

## Audit trail

Every lifecycle event that matches at least one rule inserts a
`Pending` row into `notification_attempts` before the dispatch worker
takes over. The worker terminates the row to one of `Delivered`,
`Failed`, or `Rejected` after the send attempt. Query it directly:

```bash
sqlite3 /var/lib/bithound/bithound.db \
  "SELECT id, lifecycle_kind, target_kind, status, outcome
     FROM notification_attempts
     ORDER BY rowid DESC
     LIMIT 10"
```

`status = Pending` after the fact means the worker died mid-dispatch
and the row is now an orphan audit record (V0 doesn't auto-retry;
the V0.1 retry scheduler lands in BTH-53). `status = Failed` carries
the failure reason in `outcome` — most commonly a webhook URL 4xx
(revoked Discord webhook, bad PagerDuty token) or a DNS lookup
failure.

## See also

- [Operator guide → Notification sinks](https://github.com/bithoundhq/bithound/blob/main/docs/OPERATOR_GUIDE.md#notification-sinks) —
  step-by-step credential setup for each sink.
- [`src/notifications/`](https://github.com/bithoundhq/bithound/tree/main/src/notifications) —
  the implementation, including per-sink renderers and the dispatch
  worker.
