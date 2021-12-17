use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .build_server(true)
        .file_descriptor_set_path(out_dir.join("goodmetrics_descriptor.bin"))
        .compile(&["proto/metrics/goodmetrics.proto"], &["proto"])
        .unwrap();
}
