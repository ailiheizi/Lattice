fn main() {
    println!("cargo:rerun-if-changed=../../proto/identity.proto");
    println!("cargo:rerun-if-changed=../../proto/message.proto");
    println!("cargo:rerun-if-changed=../../proto/group.proto");
    println!("cargo:rerun-if-changed=../../proto/transport.proto");
    prost_build::Config::new()
        .compile_protos(
            &[
                "../../proto/identity.proto",
                "../../proto/message.proto",
                "../../proto/group.proto",
                "../../proto/transport.proto",
            ],
            &["../../proto"],
        )
        .expect("Failed to compile protobuf files");
}
