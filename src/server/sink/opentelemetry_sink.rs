use std::collections::BTreeMap;
use std::time::Duration;

use tokio::time::{sleep, timeout_at, Instant};
use tonic::transport::Channel;

use crate::proto::channel_connection::get_channel;
use crate::proto::opentelemetry::collector::metrics::v1::metrics_service_client::MetricsServiceClient;
use crate::proto::opentelemetry::collector::metrics::v1::ExportMetricsServiceRequest;
use crate::proto::opentelemetry::common::v1::{
    any_value, AnyValue, InstrumentationLibrary, KeyValue,
};
use crate::proto::opentelemetry::metrics::v1::{
    metric, number_data_point, summary_data_point, AggregationTemporality, Gauge, Histogram,
    HistogramDataPoint, InstrumentationLibraryMetrics, Metric, NumberDataPoint, ResourceMetrics,
    Summary, SummaryDataPoint,
};
use crate::server::sink::sink_error::StringError;

use super::{metricssendqueue::MetricsReceiveQueue, sink_error::SinkError};

pub struct OtelSender {
    rx: MetricsReceiveQueue,
    client: MetricsServiceClient<Channel>,
}

impl OtelSender {
    pub async fn new_connection(
        opentelemetry_endpoint: &str,
        rx: MetricsReceiveQueue,
    ) -> Result<OtelSender, SinkError> {
        let client = match get_channel(opentelemetry_endpoint).await {
            Ok(channel) => MetricsServiceClient::new(channel),
            Err(e) => {
                return Err(SinkError::StringError(StringError {
                    message: format!("Could not get an otel channel: {:?}", e),
                }))
            }
        };

        Ok(OtelSender { rx, client })
    }

    pub async fn consume_stuff(mut self) -> Result<u32, SinkError> {
        log::info!("started opentelemetry consumer");

        while let Some(mut batch) = self.rx.recv().await {
            log::info!("Sender woke. Trying to collect a batch...");
            sleep(Duration::from_secs(5)).await;

            let deadline = Instant::now() + Duration::from_secs(1);
            let mut api_calls: u32 = 1;
            while let Ok(Some(mut extras)) = timeout_at(deadline, self.rx.recv()).await {
                api_calls += 1;
                batch.append(&mut extras);
            }
            let export_metrics: Vec<Metric> = batch.into_iter()
                .flat_map(|datum| {
                    let dimensions: Vec<KeyValue> = datum.dimensions.into_iter()
                        .filter_map(|(name, dimension)| {
                            dimension.value.map(|value| {
                                KeyValue {
                                    key: name,
                                    value: Some(AnyValue { value: Some(match value {
                                        crate::proto::goodmetrics::dimension::Value::String(s) => any_value::Value::StringValue(s),
                                        crate::proto::goodmetrics::dimension::Value::Number(n) => any_value::Value::IntValue(n as i64),
                                        crate::proto::goodmetrics::dimension::Value::Boolean(b) => any_value::Value::BoolValue(b),
                                    }) }),
                                }
                            })
                        })
                        .collect();
                    datum.measurements.into_iter()
                        .filter_map(|(name, measurement)| {
                            // Data::Gauge(()) {
                            // }
                            measurement.value.map(|value| {
                                Metric {
                                    // So yeah, this splays all your metrics across a shared namespace because prometheus / otel.
                                    name: format!("{metric_name}_{measurement_name}", metric_name = datum.metric, measurement_name=name),
                                    description: "goodmetrics compatibility conversion".to_string(),
                                    unit: "1".to_string(),
                                    data: Some(match value {
                                        crate::proto::goodmetrics::measurement::Value::I64(i) => metric::Data::Gauge(Gauge {
                                            data_points: vec![
                                                int_data_point(i, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                        crate::proto::goodmetrics::measurement::Value::I32(i) => metric::Data::Gauge(Gauge {
                                            data_points: vec![
                                                int_data_point(i as i64, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                        crate::proto::goodmetrics::measurement::Value::F64(f) => metric::Data::Gauge(Gauge {
                                            data_points: vec![
                                                float_data_point(f, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                        crate::proto::goodmetrics::measurement::Value::F32(f) => metric::Data::Gauge(Gauge {
                                            data_points: vec![
                                                float_data_point(f as f64, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                        crate::proto::goodmetrics::measurement::Value::StatisticSet(ss) => metric::Data::Summary(Summary {
                                            data_points: vec![
                                                // Well, this is the closest thing in opentelemetry. Summaries are _terrible_ though because
                                                // they encourage the incredibly error-prone practice of recording quantiles from the source.
                                                summary_data_point(ss, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                        crate::proto::goodmetrics::measurement::Value::Histogram(h) => metric::Data::Histogram(Histogram {
                                            aggregation_temporality: AggregationTemporality::Delta as i32,
                                            data_points: vec![
                                                // Well, this is the closest thing in opentelemetry. Summaries are _terrible_ though because
                                                // they encourage the incredibly error-prone practice of recording quantiles from the source.
                                                histogram_data_point(h, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                    }),
                                }
                            })
                        })
                        .collect::<Vec<Metric>>()
                })
                .collect();
            match self
                .client
                .export(ExportMetricsServiceRequest {
                    resource_metrics: vec![ResourceMetrics {
                        resource: None,
                        schema_url: "".to_string(),
                        instrumentation_library_metrics: vec![InstrumentationLibraryMetrics {
                            instrumentation_library: Some(InstrumentationLibrary {
                                name: "goodmetrics".to_string(),
                                version: "42".to_string(),
                            }),
                            schema_url: "".to_string(),
                            metrics: export_metrics,
                        }],
                    }],
                })
                .await
            {
                Ok(response) => {
                    log::info!(
                        "Sent {} batched calls to otel. Response: {:?}",
                        api_calls,
                        response
                    );
                }
                Err(error) => {
                    log::error!("Error from otel: {:?}", error);
                }
            }
        }

        Ok(1)
    }
}

fn int_data_point(i: i64, nano_time: u64, dimensions: &[KeyValue]) -> NumberDataPoint {
    NumberDataPoint {
        attributes: dimensions.to_owned(),
        start_time_unix_nano: 0,
        time_unix_nano: nano_time,
        exemplars: vec![],
        flags: 0,
        value: Some(number_data_point::Value::AsInt(i)),
    }
}

fn float_data_point(f: f64, nano_time: u64, dimensions: &[KeyValue]) -> NumberDataPoint {
    NumberDataPoint {
        attributes: dimensions.to_owned(),
        start_time_unix_nano: 0,
        time_unix_nano: nano_time,
        exemplars: vec![],
        flags: 0,
        value: Some(number_data_point::Value::AsDouble(f)),
    }
}

fn summary_data_point(
    ss: crate::proto::goodmetrics::StatisticSet,
    nano_time: u64,
    dimensions: &[KeyValue],
) -> SummaryDataPoint {
    SummaryDataPoint {
        attributes: dimensions.to_owned(),
        start_time_unix_nano: 0,
        time_unix_nano: nano_time,
        flags: 0,
        count: ss.samplecount as u64,
        sum: ss.samplesum,
        quantile_values: vec![
            summary_data_point::ValueAtQuantile {
                quantile: 0_f64,
                value: ss.minimum,
            },
            summary_data_point::ValueAtQuantile {
                quantile: 1_f64,
                value: ss.maximum,
            },
        ],
    }
}

fn histogram_data_point(
    h: crate::proto::goodmetrics::Histogram,
    nano_time: u64,
    dimensions: &[KeyValue],
) -> HistogramDataPoint {
    let buckets: BTreeMap<i64, i64> = h.buckets.into_iter().collect();
    HistogramDataPoint {
        attributes: dimensions.to_owned(),
        start_time_unix_nano: 0,
        time_unix_nano: nano_time,
        exemplars: vec![],
        flags: 0,
        count: buckets.iter().map(|(_bucket, count)| *count as u64).sum(),

        // Sum is not faithfully maintained in goodmetrics. It's approximate, and over-estimated.
        sum: buckets
            .iter()
            .map(|(bucket, count)| (bucket * count) as f64)
            .sum(),

        bucket_counts: buckets
            .iter()
            .map(|(_bucket, count)| *count as u64)
            .collect(),
        explicit_bounds: buckets
            .iter()
            .map(|(bucket, _count)| *bucket as f64)
            .collect(),
    }
}
