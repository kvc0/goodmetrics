use tonic::{transport::Server};

use std::{net::SocketAddr, cmp::min};
use tokio::net::TcpListener;

use metrics::{metrics_server::{MetricsServer, Metrics}, MetricsRequest, MetricsReply};
pub mod metrics {
    tonic::include_proto!("goodmetrics");
}

#[derive(Debug, Default)]
pub struct GoodMetricsServer {}

#[tonic::async_trait]
impl Metrics for GoodMetricsServer {
    async fn send_metrics(
        &self,
        request: tonic::Request<MetricsRequest>,
    ) -> Result<tonic::Response<MetricsReply>, tonic::Status> {
        log::debug!("request: {:?}", request);

        return Err(tonic::Status::unimplemented("it is not implemented"));
    }
}

async fn serve(listen_socket_address: &String) {
    let address: std::net::SocketAddr = listen_socket_address.parse().unwrap();
    let socket = socket2::Socket::new(
        match address {
            SocketAddr::V4(_) => socket2::Domain::IPV4,
            SocketAddr::V6(_) => socket2::Domain::IPV6,
        },
        socket2::Type::STREAM,
        None,
    )
    .unwrap();

    socket.set_reuse_address(true).unwrap();
    socket.set_reuse_port(true).unwrap();
    socket.set_nonblocking(true).unwrap();
    socket.bind(&address.into()).unwrap();
    socket.listen(8192).unwrap();

    let listener = TcpListener::from_std(socket.into()).unwrap();
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    let one_server_thread = GoodMetricsServer::default();
    let grpc_server = MetricsServer::new(one_server_thread);
    Server::builder()
        .add_service(grpc_server)
        .serve_with_incoming(incoming)
        .await
        .unwrap();
}

use serde_derive::Deserialize;
use structopt::StructOpt;
use structopt_toml::StructOptToml;

#[derive(Debug, Deserialize, StructOpt, StructOptToml)]
#[serde(default)]
struct Options {
    #[structopt(long)] config: Option<String>,
    #[structopt(long, default_value = "0.0.0.0:9573")] listen_socket_address: String,
    #[structopt(long, default_value = "1")] max_threads: usize,
    #[structopt(long, default_value = "debug")] log_level: String,
}

fn get_args() -> Options {
    let command_line_args = Options::from_args();

    match command_line_args.config {
        Some(config_file_path) => {
            let toml_str = std::fs::read_to_string(config_file_path).unwrap();
            Options::from_args_with_toml(&toml_str).unwrap()
        },
        None => {
            command_line_args
        },
    }
}

fn main() {
    let args = get_args();

    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(args.log_level)
            .default_write_style_or("always"),
    )
    .init();

    let mut handlers = Vec::new();
    for i in 0..min(args.max_threads, num_cpus::get()) {
        let listen_address = args.listen_socket_address.clone();

        let h = std::thread::spawn(move || {
            log::info!("starting server thread {} listening on {}", i, listen_address);

            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(serve(&listen_address));
        });
        handlers.push(h);
    }

    for h in handlers {
        h.join().unwrap();
    }
}
