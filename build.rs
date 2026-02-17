fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/market.proto");

    tonic_build::configure()
        .build_server(true)
        .compile(&["proto/market.proto"], &["proto"])?;

    Ok(())
}
