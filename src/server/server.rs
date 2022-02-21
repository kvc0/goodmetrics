use config::options::get_args;
use servers::goodmetrics::GoodMetricsServer;
use sink::{metricssendqueue::MetricsSendQueue, postgres_sink::PostgresSender};
use tonic::transport::{Identity, Server, ServerTlsConfig};

use std::{cmp::min, net::SocketAddr, sync::Arc};
use tokio::{join, net::TcpListener};

mod config;
mod postgres_things;
mod servers;
mod sink;

mod proto;
use proto::metrics::pb::metrics_server::MetricsServer;

async fn serve(
    args: Arc<config::options::Options>,
    send_queue: MetricsSendQueue,
) -> Result<(), Box<dyn std::error::Error>> {
    let address: std::net::SocketAddr = args.listen_socket_address.parse().unwrap();
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

    let one_server_thread = GoodMetricsServer {
        metrics_sink: send_queue,
    };

    let identity = get_identity(&args).await?;

    let grpc_server = MetricsServer::new(one_server_thread);
    Server::builder()
        .tls_config(ServerTlsConfig::new().identity(identity))?
        .add_service(grpc_server)
        .serve_with_incoming(incoming)
        .await
        .unwrap();

    Ok(())
}

async fn get_identity(
    options: &config::options::Options,
) -> Result<Identity, Box<dyn std::error::Error>> {
    let identity = if !options.cert.is_empty() && !options.cert_private_key.is_empty() {
        let cert = tokio::fs::read("examples/data/tls/server.pem").await?;
        let key = tokio::fs::read("examples/data/tls/server.key").await?;

        Identity::from_pem(cert, key)
    } else {
        let subject_alt_names = vec![options.self_signed_hostname.clone()];
        let cert = rcgen::generate_simple_self_signed(subject_alt_names).unwrap();
        let certpem = cert.serialize_pem().unwrap();
        let pkpem = cert.serialize_private_key_pem();

        Identity::from_pem(certpem, pkpem)
    };

    Ok(identity)
}

fn main() {
    let args = get_args();
    console_subscriber::init();

    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(&args.log_level)
            .default_write_style_or("always"),
    )
    .init();

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run_server(args));
}

async fn run_server(args: config::options::Options) {
    let mut handlers = Vec::new();
    let args_shared = Arc::from(args);
    let (send_queue, receive_queue) = MetricsSendQueue::new();

    let mut sender =
        match PostgresSender::new_connection(&args_shared.connection_string, receive_queue).await {
            Ok(sender) => sender,
            Err(e) => {
                log::error!("failed to start server: {:?}", e);
                std::process::exit(3)
            }
        };

    // Consume stuff on a background task
    let bg_task = sender.consume_stuff();

    for i in 0..min(args_shared.max_threads, num_cpus::get()) {
        let threadlocal_args = args_shared.clone();
        let thread_send_queue = send_queue.clone();

        let h = std::thread::spawn(move || {
            log::info!(
                "starting server thread {} listening on {}",
                i,
                &threadlocal_args.listen_socket_address
            );

            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(serve(threadlocal_args, thread_send_queue))
                .unwrap();
        });
        handlers.push(h);
    }

    match join!(bg_task) {
        (Ok(_),) => log::info!("joined background task"),
        (Err(e),) => log::error!("joined background task with error: {:?}", e),
    };

    for h in handlers {
        h.join().unwrap();
    }
}
