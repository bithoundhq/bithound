fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Re-run codegen only when the vendored proto file changes (not
    // on every cargo invocation).
    println!("cargo:rerun-if-changed=src/collectors/lnd/proto/lightning.proto");

    tonic_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(
            &["src/collectors/lnd/proto/lightning.proto"],
            &["src/collectors/lnd/proto"],
        )?;

    Ok(())
}
