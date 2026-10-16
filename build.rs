use std::error::Error;
use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    if env::var_os("PROTOC").is_none() {
        unsafe {
            env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?);
        }
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    tonic_prost_build::configure()
        .file_descriptor_set_path(out_dir.join("user_descriptor.bin"))
        .compile_protos(&["proto/usr.proto"], &["proto"])?;

    Ok(())
}
