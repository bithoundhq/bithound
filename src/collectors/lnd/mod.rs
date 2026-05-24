//! LND collectors (gRPC-based).
//!
//! V0.8 ships the polling collector (`grpc_poll.rs`) and its thin
//! client wrapper (`grpc_client.rs`). Subscription streams defer to
//! V1.0 via ADR-E3.

pub mod grpc_client;

/// Generated tonic types from the vendored `lightning.proto`.
///
/// See `proto/README.md` for the pinned upstream commit. Generated at
/// build time by `build.rs` via `tonic-build`. Clippy lints are
/// suppressed wholesale here because the generated code mirrors LND's
/// upstream `.proto` conventions (naming, doc style) that aren't ours
/// to change.
#[allow(clippy::all, clippy::pedantic, clippy::nursery)]
pub mod lnrpc {
    tonic::include_proto!("lnrpc");
}
