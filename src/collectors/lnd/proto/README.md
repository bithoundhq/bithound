# Vendored LND protocol buffers

This directory vendors LND's `.proto` files so Bithound can generate
a tonic gRPC client at build time without depending on an upstream
third-party crate.

## Pinned version

- **Source repo:** [`lightningnetwork/lnd`](https://github.com/lightningnetwork/lnd)
- **Pinned tag:** [`v0.20.1-beta`](https://github.com/lightningnetwork/lnd/releases/tag/v0.20.1-beta)
  (published 2026-02-12)
- **Tagged commit SHA:** `848b72ce96eb68fa90fd4336523ca4c59bddcd4c`
- **Captured:** 2026-05-24

Pinned to a tagged release rather than a moving `master` branch so
the integration story reads cleanly to operators: "Bithound is built
against the LND `v0.20.1-beta` protocol surface; protobuf wire
compatibility means clients work against any LND server `v0.18+`."

Re-fetch this exact file with:

```bash
curl -sSL https://raw.githubusercontent.com/lightningnetwork/lnd/v0.20.1-beta/lnrpc/lightning.proto
```

## Vendored set

| File | Size | SHA-256 |
|---|---|---|
| `lightning.proto` | 176,086 bytes | `8de51253eaa478175ab21be522862cfa33c07d9dc390d7aa4544bf4220ac4f3a` |

No transitive imports. This `lightning.proto` is the stripped
variant (no `import "google/api/annotations.proto"`, no REST gateway
`option (google.api.http) = { ... }` annotations), so `tonic-build`
compiles it standalone. LND's published proto strips the REST gateway
annotations from the gRPC-only surface as of v0.18+. If a future LND
version reintroduces those imports, the vendoring set must grow to
include `google/api/*` protos or strip them locally.

Verify the SHA-256 with:

```bash
shasum -a 256 src/collectors/lnd/proto/lightning.proto
```

## Update cadence

Review the upstream `lnrpc/lightning.proto` diff against each new LND
minor release (roughly every 3-6 months). Pull updates deliberately,
not automatically:

1. Diff against the new release's `lightning.proto`.
2. If new fields the Bithound collector consumes have been added,
   land an update PR that re-vendors the file at the new tag,
   refreshes the SHA-256 in this README, and bumps the captured date.
3. If the upstream file reintroduces `import` statements or REST
   gateway annotations, decide between vendoring the supporting
   protos under `proto/google/api/` or stripping the new imports
   locally. The current variant is the stripped form; sticking with
   stripped is the cheaper path unless a Bithound feature requires
   the annotations.

## License

`lightning.proto` retains LND's MIT license header at the top of the
file (Copyright 2015-2022 Lightning Labs and The Lightning Network
Developers). The full license text is preserved in the file itself;
no separate `LICENSE` file is required alongside.
