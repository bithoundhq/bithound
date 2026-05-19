# Incident-kind schema

## TOML shape

```toml
[[kinds]]
name              = "namespace.event"             # required, unique
allowed_subjects  = ["BitcoinNode", "Host"]       # required, non-empty
allows_dimension  = false                         # required
dimension_label   = "mount_path"                  # optional, documentation
min_open_confidence = "Medium"                    # optional, default Medium
```

| Field                 | Type            | Required | Notes                                                                                  |
| --------------------- | --------------- | -------- | -------------------------------------------------------------------------------------- |
| `name`                | string          | yes      | Unique across the combined built-in and user catalogs.                                 |
| `allowed_subjects`    | array of string | yes      | Each entry must be a known [`EntitySubjectKind`](#subject-kind-names).                  |
| `allows_dimension`    | bool            | yes      | `true` requires `dimension` on every draft; `false` forbids it.                        |
| `dimension_label`     | string          | no       | Documentation only — not validated against draft contents.                              |
| `min_open_confidence` | string          | no       | One of `"Low"`, `"Medium"`, `"High"`. Default `"Medium"`. Drafts below this don't lift. |

## Subject-kind names

The full set of `EntitySubjectKind` values:

- `Host`
- `BitcoinNode`
- `BitcoinPeer`
- `LndNode`
- `LndPeer`
- `LndChannel`
- `LndInvoice`

## Validation errors

The loader emits structured errors. Listed roughly in the order an
operator is likely to see them:

| Error                  | When it fires                                                                |
| ---------------------- | ---------------------------------------------------------------------------- |
| `InvalidToml(...)`     | The file isn't valid TOML, or a field has the wrong type.                    |
| `UnknownSubjectKind`   | `allowed_subjects` contains a name that isn't an `EntitySubjectKind`.        |
| `DuplicateKind`        | Two entries in the same file (or two user-config entries) share a `name`.    |
| `CannotOverrideBuiltin`| A user-config entry shadows a built-in. User configs are additive only.      |

Per-draft validation, performed by the engine on every incoming signal:

| Error                | When it fires                                                          |
| -------------------- | ---------------------------------------------------------------------- |
| `UnknownKind`        | The draft's `kind` isn't registered.                                   |
| `DisallowedSubject`  | The draft's subject isn't in the kind's `allowed_subjects`.            |
| `DimensionRequired`  | `allows_dimension = true` and the draft has `dimension = None`.        |
| `DimensionForbidden` | `allows_dimension = false` and the draft has `dimension = Some(_)`.    |

A draft that fails validation is rejected outright: no signal
observation is persisted and no incident state is mutated.

## See also

- [Custom incidents](../operator/custom-incidents.md) — operator-facing
  walkthrough.
- [`config/default_kinds.toml`](https://github.com/bithoundhq/bithound/blob/main/config/default_kinds.toml) —
  the built-in catalog.
- [`src/incidents/kinds.rs`](https://github.com/bithoundhq/bithound/blob/main/src/incidents/kinds.rs) —
  the loader and validator.
