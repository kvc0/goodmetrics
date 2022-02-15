use lazy_static::lazy_static;
use serde::Deserialize;
use structopt::StructOpt;
use structopt_toml::StructOptToml;

use super::cli_config::default_dir;
use crate::metrics::Datum;

lazy_static! {
    static ref DEFAULT_DIR: String = default_dir();
}

#[derive(Debug, Deserialize, StructOpt, StructOptToml)]
#[serde(default)]
#[structopt(about = "Good metrics CLI client")]
pub(crate) struct Options {
    #[structopt(long, default_value = &DEFAULT_DIR)]
    pub config_file: String,
    #[structopt(long, default_value = "https://localhost:9573")]
    pub goodmetrics_server: String,
    #[structopt(long, default_value = "debug")]
    pub log_level: String,

    #[structopt(subcommand)]
    pub command: Subcommand,
}

#[derive(Debug, Deserialize, StructOpt)]
pub(crate) enum Subcommand {
    #[structopt(about = "Send measurements")]
    Send {
        #[structopt(parse(try_from_str = serde_json::from_str))]
        metrics: Vec<Datum>,
    },
    #[structopt(about = "Poll prometheus metrics")]
    PollPrometheus {
        #[structopt(default_value = "http://127.0.0.1:9090")]
        poll_endpoint: String,
        #[structopt(long, default_value = "10")]
        interval_seconds: u32,
    },
}
