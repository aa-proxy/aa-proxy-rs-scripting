fn main() {
    println!("cargo:rerun-if-changed=src/protos/WifiStartRequest.proto");
    println!("cargo:rerun-if-changed=src/protos/WifiInfoResponse.proto");
    println!("cargo:rerun-if-changed=src/protos/protos.proto");
    println!("cargo:rerun-if-changed=src/protos/ev.proto");

    protobuf_codegen::Codegen::new()
        .protoc()
        .protoc_path(&protoc_bin_vendored::protoc_bin_path().unwrap())
        .includes(&["src/protos"])
        .input("src/protos/WifiStartRequest.proto")
        .input("src/protos/WifiInfoResponse.proto")
        .input("src/protos/protos.proto")
        .input("src/protos/ev.proto")
        .cargo_out_dir("protos")
        .run_from_script();
}