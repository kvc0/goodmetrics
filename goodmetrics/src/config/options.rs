use std::collections::HashMap;

use clap::Parser;
use lazy_static::lazy_static;
use serde::Deserialize;

use super::cli_config::default_dir;
use communication::proto::goodmetrics::{Datum, Dimension};

lazy_static! {
    static ref DEFAULT_DIR: String = default_dir();
}

#[derive(Debug, Deserialize, Parser)]
//#[serde(default)]
#[clap(about = "Goodmetrics CLI client")]
pub struct Options {
    #[clap(long, default_value = &**DEFAULT_DIR)]
    pub config_file: String,
    #[clap(long, default_value = "https://localhost:9573")]
    pub goodmetrics_server: String,
    #[clap(long, default_value = "info")]
    pub log_level: String,
    #[clap(
        long,
        short,
        help = "Authorization token to use - if the remote server expects this"
    )]
    pub authorization: Option<String>,

    #[clap(subcommand)]
    pub command: Subcommand,
}

#[derive(Debug, Deserialize, Parser)]
pub enum Subcommand {
    #[clap(about = "Send measurements")]
    Send {
        #[arg(value_parser = parse_metrics)]
        metrics: Vec<Datum>,
        #[arg(
            long,
            help = "send to a goodmetrics server without validating the certificate"
        )]
        insecure: bool,
    },
    #[clap(about = "Poll prometheus metrics")]
    PollPrometheus {
        #[arg(
            help = "Prefix all the tables emitted by this prometheus reporter. This way you can put things like per-server host metrics under a host_* or node_* prefix."
        )]
        prefix: String,

        #[arg(default_value = "http://127.0.0.1:9100/metrics")]
        poll_endpoint: String,

        #[arg(long, default_value = "10")]
        interval_seconds: u32,

        #[arg(
            long,
            help = "send to a goodmetrics server without validating the certificate"
        )]
        insecure: bool,

        #[arg(long, default_value = "{}", value_parser = parse_dimensions)]
        bonus_dimensions: HashMap<String, Dimension>,
    },
}

fn parse_dimensions(value: &str) -> anyhow::Result<HashMap<String, Dimension>> {
    serde_json::from_str(value).map_err(|e| anyhow::anyhow!("could not parse dimensions: {e:?}"))
}

fn parse_metrics(value: &str) -> anyhow::Result<Datum> {
    serde_json::from_str(value).map_err(|e| anyhow::anyhow!("could not parse metrics: {e:?}"))
}
