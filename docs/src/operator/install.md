# Installation

> **Stub.** Bithound has no released binary yet; the runtime is being
> assembled ticket by ticket. This page will be filled out once the
> first end-to-end runtime ships.

For now, build from source:

```bash
git clone https://github.com/bithoundhq/bithound
cd bithound
cargo build --release
```

The binary is written to `target/release/bithound`. Once a runtime
entry-point exists, this page will document:

- supported host platforms and OS versions,
- runtime dependencies (a reachable `bitcoind` JSON-RPC endpoint, an
  optional LND with macaroon access, the local filesystem),
- where to drop the binary and the configuration file,
- how to wire it up as a `systemd` unit and what permissions it needs,
- upgrade and rollback procedure.
