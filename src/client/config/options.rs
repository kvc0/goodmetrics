use std::collections::HashMap;

use clap::Parser;
use lazy_static::lazy_static;
use serde::Deserialize;

use super::cli_config::default_dir;
use crate::proto::goodmetrics::{Datum, Dimension};

lazy_static! {
    static ref DEFAULT_DIR: String = default_dir();
}

#[derive(Debug, Deserialize, Parser)]
//#[serde(default)]
#[clap(about = "Goodmetrics CLI client")]
pub struct Options {
    #[clap(long, default_value = &DEFAULT_DIR)]
    pub config_file: String,
    #[clap(long, default_value = "https://localhost:9573")]
    pub goodmetrics_server: String,
    #[clap(long, default_value = "info")]
    pub log_level: String,

    #[clap(subcommand)]
    pub command: Subcommand,
}

#[derive(Debug, Deserialize, Parser)]
pub enum Subcommand {
    #[clap(about = "Send measurements")]
    Send {
        #[clap(parse(try_from_str = serde_json::from_str))]
        metrics: Vec<Datum>,
    },
    #[clap(about = "Poll prometheus metrics")]
    PollPrometheus {
        #[clap(
            help = "Prefix all the tables emitted by this prometheus reporter. This way you can put things like per-server host metrics under a host_* or node_* prefix."
        )]
        prefix: String,

        #[clap(default_value = "http://127.0.0.1:9100/metrics")]
        poll_endpoint: String,

        #[clap(long, default_value = "10")]
        interval_seconds: u32,

        #[clap(long, default_value = "{}", parse(try_from_str = serde_json::from_str))]
        bonus_dimensions: HashMap<String, Dimension>,
    },
}
