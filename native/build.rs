use cirnet_contracts_native::hardware_v2;
use prost::Message;
use prost_types::FileDescriptorSet;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let canonical = FileDescriptorSet::decode(hardware_v2::FILE_DESCRIPTOR_SET)?;
    let legacy = FileDescriptorSet::decode(hardware_v2::LEGACY_FILE_DESCRIPTOR_SET)?;
    tonic_prost_build::configure()
        .build_client(false)
        .extern_path(
            ".cirnet.hardware.v2",
            "::cirnet_contracts_native::hardware_v2",
        )
        .compile_fds(canonical)?;
    tonic_prost_build::configure()
        .build_client(false)
        .extern_path(
            ".netcircuit.hardware.v2",
            "::cirnet_contracts_native::hardware_v2",
        )
        .compile_fds(legacy)?;
    println!("cargo:rerun-if-changed=native/preview.proto");
    let mut config = tonic_prost_build::Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);
    tonic_prost_build::configure()
        .build_client(false)
        .compile_with_config(config, &["native/preview.proto"], &["native"])?;
    Ok(())
}
