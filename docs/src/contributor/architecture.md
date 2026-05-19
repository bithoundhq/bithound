# Architecture

> This page gives the contributor-facing tour. The authoritative
> reference is `SPEC.md`; this is the index into it.

## Runtime topology

Bithound is structured as a **single-writer pipeline** running inside
one tokio task. Collectors run as their own tasks and feed a bounded
`mpsc::channel` of `ObservationBatch`es; everything downstream of the
channel is single-consumer so `&mut self` works without locks.

```text
collectors → ObservationBatch → observation store
                              ↓
                         read models (apply &mut self)
                              ↓
                         diagnostic rules → IncidentSignalDraft
                                          ↓
                                    IncidentEngine.handle()
                                          ↓
                                    HandleOutcome { signal_observation,
                                                    touched_incident,
                                                    lifecycle_events }
                                          ↓
                                    incident_repo.save (write-through)
                                          ↓
                                    Notifier.dispatch → Telegram /
                                                        Discord /
                                                        webhook
```

## Module map

| Path                  | Role                                                                  |
| --------------------- | --------------------------------------------------------------------- |
| `src/shared/`         | ID newtypes, `EntityRef`, `EntitySubjectKind`, `EvidenceRef`           |
| `src/observations/`   | Observation envelope + 10 payload variants                            |
| `src/collectors/`     | `PollingCollector` / `SubscriptionCollector` traits                    |
| `src/read_models/`    | Six trait surfaces + the `ReadModelStore`                              |
| `src/diagnostics/`    | `DiagnosticRule` trait, `IncidentSignalDraft`                          |
| `src/incidents/`      | `Incident`, `IncidentFingerprint`, `IncidentEngine`, kind registry     |
| `src/notifications/`  | `Notifier` + Telegram, Discord, webhook target adapters                |
| `src/runtime/`        | Supervisor + single-consumer task (designed, not yet present)          |
| `src/storage/`        | `sqlx`-backed implementations of the trait surfaces                    |
| `src/config/`         | TOML + `clap` CLI (designed, not yet present)                          |
| `migrations/`         | `sqlx` migration files                                                 |

## Vocabulary

When writing new code, prefer the post-design vocabulary:

- *collector* (not "probe runner")
- *observation* (not "raw probe result")
- *projection* / *read model* (not "reducer" / "snapshot")
- *incident engine* (not "incident detector")
- *notifier* (not "consumer" / "exporter")

If you encounter the older vocabulary in a comment or doc, fix it as a
drive-by.

## Key invariants

- **The incident engine is single-writer.** Don't add a second mutator
  of the `open_incidents` map. All mutations route through
  `engine.handle()`.
- **Rules own their own hysteresis.** The engine treats every `Active`
  draft as immediate-open. Rules look back through read models to
  decide when to emit.
- **Observations are append-only facts.** Never mutate an `Observation`
  after construction; produce a new one if you need to record a change.
- **`Suppressed` in `IncidentStatus` is reserved for a future
  iteration.** The current engine never sets it. Suppression today is
  notifier-side.
