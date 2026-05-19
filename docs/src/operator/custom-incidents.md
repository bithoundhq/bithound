# Custom incidents

Operators can extend Bithound's incident-kind catalog by pointing the
sidecar at a TOML file of their own. User-supplied kinds are **additive**:
they sit alongside the built-in catalog. The sidecar will refuse to
start if a user kind tries to override a built-in.

## Schema

Each entry is a `[[kinds]]` table:

```toml
[[kinds]]
name              = "operator.my_custom_check"   # required, must be unique
allowed_subjects  = ["BitcoinNode"]               # required, see "Subject names"
allows_dimension  = false                         # required (true | false)
dimension_label   = "payment_hash"                # optional, documentation
min_open_confidence = "Medium"                    # optional, default "Medium"
```

### `name`

A free-form string. Convention: namespace by detector domain
(`operator.<your_org>.<check>`). Names must be unique across the
combined built-in and user catalogs.

### `allowed_subjects`

A list of subject-kind names. A draft for this kind whose subject does
not match one of these is rejected.

Valid names: `Host`, `BitcoinNode`, `BitcoinPeer`, `LndNode`, `LndPeer`,
`LndChannel`, `LndInvoice`.

### `allows_dimension`

- `false`: the kind dedups by subject alone. Drafts with a `dimension`
  set are rejected as `DimensionForbidden`.
- `true`: the kind requires a `dimension`. Drafts without one are
  rejected as `DimensionRequired`. Use this for kinds where one subject
  can have multiple concurrent instances (e.g. multiple stuck HTLCs on
  one channel, multiple full disks on one host).

### `dimension_label`

Free-form documentation string indicating what your rule will put in
`dimension` (e.g. `mount_path`, `payment_hash`). Bithound does **not**
validate the contents of `dimension` against this label — it's there for
human readers.

### `min_open_confidence`

Drafts with a `confidence` strictly below this threshold are still
persisted as signal observations (so they show up in dashboards and
read models), but the incident-lift step is skipped — no incident is
opened. Default `"Medium"`. Valid values: `"Low"`, `"Medium"`, `"High"`.

## Example

```toml
# /etc/bithound/custom_kinds.toml

[[kinds]]
name = "operator.lnd_fee_drift"
allowed_subjects = ["LndChannel"]
allows_dimension = false
min_open_confidence = "Medium"

[[kinds]]
name = "operator.bitcoin_zmq_lag"
allowed_subjects = ["BitcoinNode"]
allows_dimension = true
dimension_label = "topic"
```

Wire it up in your main config:

```toml
[incidents]
kinds_config_path = "/etc/bithound/custom_kinds.toml"
```

## Validation timing

- **Startup.** The full catalog is validated before the sidecar takes
  any traffic. A duplicate name, an attempt to override a built-in, or
  an unknown subject-kind name causes the sidecar to fail to start with
  a structured error.
- **Per-draft.** Every emitted signal is validated against the
  registry. A failure rejects the draft outright — no signal
  observation is persisted and no incident state is mutated.

## What you still need to provide

Registering a kind tells Bithound the kind exists. To actually emit
incidents under that kind you also need a diagnostic rule. Rule authoring
is a contributor-side topic — see the
[contributor guide](../contributor/overview.md).
