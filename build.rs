fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("proto/bandwidth.proto")?;
    tonic_build::compile_protos("proto/core.proto")?;
    Ok(())
}
