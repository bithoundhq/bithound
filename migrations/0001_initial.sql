-- ADR-P1: SQLite backend via sqlx — initial schema.
-- STRICT (SQLite 3.37+) enforces column type rigor; no silent text-to-int coercion.

CREATE TABLE observations (
    id              BLOB PRIMARY KEY,
    observed_at     INTEGER NOT NULL,
    received_at     INTEGER,
    subject_kind    TEXT NOT NULL,
    subject_id      TEXT NOT NULL,
    sidecar_id      BLOB NOT NULL,
    collector_id    TEXT NOT NULL,
    integration     TEXT NOT NULL,
    instance_label  TEXT NOT NULL,
    origin          TEXT NOT NULL,
    payload_kind    TEXT NOT NULL,
    payload_json    TEXT NOT NULL,
    attributes_json TEXT NOT NULL
) STRICT;

CREATE INDEX idx_obs_observed_at  ON observations (observed_at DESC);
CREATE INDEX idx_obs_subject      ON observations (subject_kind, subject_id, observed_at DESC);
CREATE INDEX idx_obs_payload_kind ON observations (payload_kind, observed_at DESC);

CREATE TABLE incidents (
    id            BLOB PRIMARY KEY,
    fingerprint   TEXT NOT NULL,
    kind          TEXT NOT NULL,
    subject_kind  TEXT NOT NULL,
    subject_id    TEXT NOT NULL,
    severity      TEXT NOT NULL,
    status        TEXT NOT NULL,
    opened_at     INTEGER NOT NULL,
    updated_at    INTEGER NOT NULL,
    resolved_at   INTEGER,
    incident_json TEXT NOT NULL
) STRICT;

CREATE INDEX idx_inc_fingerprint ON incidents (fingerprint);
CREATE INDEX idx_inc_status      ON incidents (status);
CREATE INDEX idx_inc_resolved_at ON incidents (resolved_at);

CREATE TABLE suppression_rules (
    id         BLOB PRIMARY KEY,
    fingerprint TEXT NOT NULL,
    until      INTEGER,
    reason     TEXT NOT NULL,
    actor      TEXT NOT NULL,
    created_at INTEGER NOT NULL
) STRICT;

CREATE INDEX idx_supp_fingerprint ON suppression_rules (fingerprint, until);
CREATE INDEX idx_supp_until       ON suppression_rules (until);

-- ADR-P3 §P3.2: notification attempts (audit-only in V0; retry queue is V0.1).
-- Retry columns are retained for forward compatibility and unused under V0.
CREATE TABLE notification_attempts (
    id                BLOB PRIMARY KEY,
    rule_id           BLOB NOT NULL,
    incident_id       BLOB NOT NULL,
    lifecycle_kind    TEXT NOT NULL,

    target_kind       TEXT NOT NULL,
    target_summary    TEXT NOT NULL,

    status            TEXT NOT NULL,
    attempt_number    INTEGER NOT NULL,
    parent_attempt_id BLOB,

    next_retry_at     INTEGER,

    outcome_kind      TEXT,
    outcome_json      TEXT,
    external_ref_json TEXT,

    attempted_at      INTEGER NOT NULL,
    completed_at      INTEGER
) STRICT;

CREATE INDEX idx_attempts_incident_id        ON notification_attempts (incident_id);
CREATE INDEX idx_attempts_rule_id            ON notification_attempts (rule_id);
CREATE INDEX idx_attempts_status_next_retry  ON notification_attempts (status, next_retry_at);
CREATE INDEX idx_attempts_attempted_at       ON notification_attempts (attempted_at DESC);
