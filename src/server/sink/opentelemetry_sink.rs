// use super::{metricssendqueue::MetricsReceiveQueue, sink_error::SinkError};

// pub struct OtelSender {
//     rx: MetricsReceiveQueue,
// }

// impl OtelSender {
//     pub async fn new_connection(
//         connection_string: &str,
//         rx: MetricsReceiveQueue,
//     ) -> Result<OtelSender, SinkError> {
//         // get_client();
//         todo!()
//     }

//     pub async fn consume_stuff(mut self) -> Result<u32, SinkError> {
//         log::info!("started postgres consumer");
//         todo!()
//     }
// }
