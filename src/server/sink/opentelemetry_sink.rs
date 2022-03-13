// use crate::proto::channel_connection::get_channel;

// use super::{metricssendqueue::MetricsReceiveQueue, sink_error::SinkError};

// pub struct OtelSender {
//     rx: MetricsReceiveQueue,
// }

// impl OtelSender {
//     pub async fn new_connection(
//         opentelemetry_endpoint: &str,
//         rx: MetricsReceiveQueue,
//     ) -> Result<OtelSender, SinkError> {
//         let channel = get_channel(opentelemetry_endpoint);

//         todo!()
//     }

//     pub async fn consume_stuff(mut self) -> Result<u32, SinkError> {
//         log::info!("started postgres consumer");
//         todo!()
//     }
// }
