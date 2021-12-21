use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    tonic_build::configure()
        .build_server(true)
        .type_attribute(".", "#[derive(serde::Deserialize, serde::Serialize)]")
        // .type_attribute("goodmetrics.Measurement.MeasurementType", "use serde::{Deserialize};")
        // .type_attribute("goodmetrics.Datum", derivation)
        // .type_attribute("goodmetrics.Dimension", derivation)
        // .type_attribute("goodmetrics.Measurement", derivation)
        // .type_attribute("goodmetrics.Measurement.measurement_type", derivation)
        .file_descriptor_set_path(out_dir.join("goodmetrics_descriptor.bin"))
        .compile(&["proto/metrics/goodmetrics.proto"], &["proto"])
        .unwrap();
}
