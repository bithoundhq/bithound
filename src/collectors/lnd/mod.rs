//! LND collectors (gRPC-based).
//!
//! Surface stubbed for v0.8; the polling collector lands in
//! `grpc_poll.rs` and the client wrapper in `grpc_client.rs` as
//! follow-on tickets implement the V0.8 LND wedge.

/// Generated tonic types from the vendored `lightning.proto`.
///
/// See `proto/README.md` for the pinned upstream commit. Generated at
/// build time by `build.rs` via `tonic-build`.
pub mod lnrpc {
    tonic::include_proto!("lnrpc");
}
