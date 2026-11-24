use cirnet_contracts_native::hardware_v2;
use prost::Message;
use prost_types::FileDescriptorSet;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::path::PathBuf;

fn protoc() -> Result<PathBuf, Box<dyn Error>> {
    println!("cargo:rerun-if-env-changed=PROTOC");
    println!("cargo:rerun-if-env-changed=CIRNET_PROTOC_SHA256");
    match (
        std::env::var_os("PROTOC"),
        std::env::var_os("CIRNET_PROTOC_SHA256"),
    ) {
        (None, None) => Ok(protoc_bin_vendored::protoc_bin_path()?),
        (Some(path), Some(expected)) => {
            let path = PathBuf::from(path);
            if !path.is_absolute() || !path.is_file() {
                return Err("PROTOC must name an absolute compiler file".into());
            }
            println!("cargo:rerun-if-changed={}", path.display());
            let actual = format!("{:x}", Sha256::digest(std::fs::read(&path)?));
            if expected != actual.as_str() {
                return Err("PROTOC SHA-256 differs from CIRNET_PROTOC_SHA256".into());
            }
            Ok(path)
        }
        _ => Err("Set both PROTOC and CIRNET_PROTOC_SHA256 to select a compiler".into()),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let canonical = FileDescriptorSet::decode(hardware_v2::FILE_DESCRIPTOR_SET)?;
    tonic_prost_build::configure()
        .build_client(false)
        .extern_path(
            ".cirnet.hardware.v2",
            "::cirnet_contracts_native::hardware_v2",
        )
        .compile_fds(canonical)?;
    println!("cargo:rerun-if-changed=proto/preview.proto");
    let mut config = tonic_prost_build::Config::new();
    config.protoc_executable(protoc()?);
    tonic_prost_build::configure()
        .build_client(false)
        .compile_with_config(config, &["proto/preview.proto"], &["proto"])?;
    Ok(())
}
