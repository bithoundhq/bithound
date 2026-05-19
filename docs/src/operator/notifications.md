# Notifications

> **Stub.** The notifier subsystem is implemented but not yet wired to
> a runtime. This page will document target setup once the sidecar is
> end-to-end runnable.

Bithound's notifier supports three target types:

- **Telegram** — a bot you create, paired with each chat that should
  receive alerts. Pairing is operator-driven; the sidecar never accepts
  unrequested chats.
- **Discord** — webhooks dropped into channels (server-owner driven),
  with severity-aware embeds.
- **Webhook** — a generic HTTP POST target with optional HMAC signing,
  useful for routing into existing alerting or ticketing pipelines.

Per-target severity filters, rate limits, and suppression rules let
operators dial how chatty each channel is.
