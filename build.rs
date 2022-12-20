use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .build_server(true)
        .type_attribute(".", "#[derive(serde::Deserialize, serde::Serialize)]")
        .file_descriptor_set_path(out_dir.join("goodmetrics_descriptor.bin"))
        .compile(&["proto/metrics/goodmetrics.proto"], &["proto"])
        .unwrap();

    tonic_build::configure()
        .build_server(false)
        // .type_attribute(".", "#[derive(Debug)]")
        .compile(
            &[
                "proto/opentelemetry/metrics/v1/metrics.proto",
                "proto/opentelemetry/collector/metrics/v1/metrics.proto",
            ],
            &["proto/opentelemetry"],
        )
        .unwrap();
}
