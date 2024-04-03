use communication::proto::goodmetrics::metrics_server::MetricsServer;
use config::options::Options;
use sink::metricssendqueue::{MetricsReceiveQueue, MetricsSendQueue};
use sink::opentelemetry_sink::OtelSender;
use sink::postgres_sink::PostgresSender;
use sink::sink_error::SinkError;
use tonic::transport::{Identity, Server, ServerTlsConfig};

use std::collections::HashSet;
use std::{cmp::min, net::SocketAddr};
use tokio::net::TcpListener;

use crate::config::options::get_args;
use crate::servers::goodmetrics::GoodmetricsServer;

mod config;
mod postgres_things;
mod servers;
mod sink;

async fn serve(
    args: Options,
    send_queue: MetricsSendQueue,
) -> Result<(), Box<dyn std::error::Error>> {
    let address: std::net::SocketAddr = args.listen_socket_address.parse()?;
    let socket = socket2::Socket::new(
        match address {
            SocketAddr::V4(_) => socket2::Domain::IPV4,
            SocketAddr::V6(_) => socket2::Domain::IPV6,
        },
        socket2::Type::STREAM,
        None,
    )?;

    socket.set_reuse_address(true)?;
    socket.set_reuse_port(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&address.into())?;
    socket.listen(8192)?;

    let listener = TcpListener::from_std(socket.into()).expect("tcp listener can be made");
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);

    let one_server_thread = GoodmetricsServer {
        metrics_sink: send_queue,
    };

    let identity = get_identity(&args).await?;

    let keys: HashSet<String> = args
        .api_keys
        .iter()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect();

    let mut server_builder =
        Server::builder().tls_config(ServerTlsConfig::new().identity(identity))?;

    let service_router = if keys.is_empty() {
        log::info!("configuring unauthorized metrics server");
        server_builder.add_service(MetricsServer::new(one_server_thread))
    } else {
        log::info!(
            "configuring authorized metrics server with {} access keys",
            keys.len()
        );
        server_builder.add_service(MetricsServer::with_interceptor(
            one_server_thread,
            move |request: tonic::Request<()>| match request.metadata().get("authorization") {
                Some(authorization_header) => match authorization_header.to_str() {
                    Ok(token) => {
                        if keys.contains(token) {
                            Ok(request)
                        } else {
                            Err(tonic::Status::unauthenticated(
                                "authorization token is not allowed",
                            ))
                        }
                    }
                    Err(e) => Err(tonic::Status::invalid_argument(format!(
                        "authorization token is not well-formed: {e:?}"
                    ))),
                },
                None => Err(tonic::Status::unauthenticated(
                    "authorization token is required",
                )),
            },
        ))
    };
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(communication::proto::goodmetrics::DESCRIPTOR)
        .build()?;
    let service_router = service_router.add_service(reflection);

    log::info!("entering serve function");
    service_router
        .serve_with_incoming(incoming)
        .await
        .expect("service comples");

    Ok(())
}

async fn get_identity(options: &Options) -> Result<Identity, Box<dyn std::error::Error>> {
    let identity = if !options.cert.is_empty() && !options.cert_private_key.is_empty() {
        let cert = tokio::fs::read(&options.cert).await?;
        let key = tokio::fs::read(&options.cert_private_key).await?;

        Identity::from_pem(cert, key)
    } else {
        let subject_alt_names = vec![options.self_signed_hostname.clone()];
        let cert = rcgen::generate_simple_self_signed(subject_alt_names)
            .expect("rcgen generates a certificate");
        let certpem = cert.serialize_pem().expect("rcgen creates a valid pem");
        let pkpem = cert.serialize_private_key_pem();

        Identity::from_pem(certpem, pkpem)
    };

    Ok(identity)
}

fn main() {
    let args = get_args();
    if args.tokio_console {
        console_subscriber::init();
    }

    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(&args.log_level)
            .default_write_style_or(&args.log_level),
    )
    .init();

    log::info!("args: {:?}", args);

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime can be made")
        .block_on(run_server(args));
}

async fn run_server(args: Options) {
    let mut handlers = Vec::new();
    let args_shared = args;
    let (send_queue, receive_queue) = MetricsSendQueue::new();

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
                .expect("runtime can be made")
                .block_on(serve(threadlocal_args, thread_send_queue))
                .expect("server completes");
        });
        handlers.push(h);
    }

    if let Some(connection_string_arg) = &args_shared.connection_string {
        let connection_string = connection_string_arg.clone();
        let threadlocal_args = args_shared.clone();
        let bg_handle = std::thread::spawn(move || {
            // Consume stuff on a background task
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime can be made")
                .block_on(consume_postgres(
                    connection_string,
                    receive_queue,
                    threadlocal_args,
                ))
                .expect("postgres sender completes");
        });
        handlers.push(bg_handle);
    }

    let insecure_otlp = args_shared.otlp_insecure;
    if let Some(otlp_remote_arg) = &args_shared.otlp_remote {
        let cloned_queue = MetricsReceiveQueue {
            rx: send_queue.tx.subscribe(),
        };
        let otlp_remote = otlp_remote_arg.clone();
        let bg_handle = std::thread::spawn(move || {
            // Consume stuff on a background task
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime can be made")
                .block_on(consume_otel(otlp_remote, cloned_queue, insecure_otlp))
                .expect("otel sender completes");
        });
        handlers.push(bg_handle);
    }

    for h in handlers {
        h.join().expect("all handles join gracefully");
    }
}

async fn consume_postgres(
    connection_string: String,
    receive_queue: MetricsReceiveQueue,
    options: Options,
) -> Result<(), SinkError> {
    let sender =
        match PostgresSender::new_connection(&connection_string, receive_queue, options).await {
            Ok(sender) => sender,
            Err(e) => {
                log::error!("failed to start postgres sender: {:?}", e);
                std::process::exit(3)
            }
        };
    sender.consume_stuff().await?;
    Ok(())
}

async fn consume_otel(
    opentelemetry_endpoint: String,
    receive_queue: MetricsReceiveQueue,
    insecure: bool,
) -> Result<(), SinkError> {
    let sender =
        match OtelSender::new_connection(&opentelemetry_endpoint, receive_queue, insecure).await {
            Ok(sender) => sender,
            Err(e) => {
                log::error!("failed to start otel sender: {:?}", e);
                std::process::exit(3)
            }
        };
    sender.consume_stuff().await?;
    Ok(())
}
