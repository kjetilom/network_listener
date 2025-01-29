use std::io::Result;
fn main() -> Result<()> {
    prost_build::compile_protos(&["src/bandwidth.proto"], &["src/"])?;
    Ok(())
}