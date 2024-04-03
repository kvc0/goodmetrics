mod channel_connection;

pub use channel_connection::get_channel;
pub use channel_connection::ChannelType;

#[allow(clippy::unwrap_used)]
pub mod proto {
    pub mod goodmetrics {
        tonic::include_proto!("goodmetrics");
        pub const DESCRIPTOR: &[u8] = tonic::include_file_descriptor_set!("goodmetrics_descriptor");
    }

    pub mod opentelemetry {
        pub mod collector {
            pub mod metrics {
                pub mod v1 {
                    tonic::include_proto!("opentelemetry.proto.collector.metrics.v1");
                }
            }
        }
        pub mod common {
            pub mod v1 {
                tonic::include_proto!("opentelemetry.proto.common.v1");
            }
        }
        pub mod metrics {
            pub mod v1 {
                tonic::include_proto!("opentelemetry.proto.metrics.v1");
            }
        }
        pub mod resource {
            pub mod v1 {
                tonic::include_proto!("opentelemetry.proto.resource.v1");
            }
        }
    }
}
