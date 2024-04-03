use std::collections::BTreeMap;
use std::time::Duration;

use communication::proto::goodmetrics;
use communication::{get_channel, ChannelType};
use tokio::time::{sleep, timeout_at, Instant};

use communication::proto::opentelemetry;
use communication::proto::opentelemetry::collector::metrics::v1::metrics_service_client::MetricsServiceClient;
use communication::proto::opentelemetry::collector::metrics::v1::ExportMetricsServiceRequest;
use communication::proto::opentelemetry::common::v1::{
    any_value, AnyValue, InstrumentationLibrary, KeyValue,
};

use super::sink_error::StringError;
use super::{metricssendqueue::MetricsReceiveQueue, sink_error::SinkError};

use opentelemetry::metrics::v1 as opentelemetry_metrics;

pub struct OtelSender {
    rx: MetricsReceiveQueue,
    client: MetricsServiceClient<ChannelType>,
}

impl OtelSender {
    pub async fn new_connection(
        opentelemetry_endpoint: &str,
        rx: MetricsReceiveQueue,
        insecure: bool,
    ) -> Result<OtelSender, SinkError> {
        let client = match get_channel(opentelemetry_endpoint, insecure).await {
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
            let export_metrics: Vec<opentelemetry_metrics::Metric> = batch.into_iter()
                .flat_map(|datum| {
                    let dimensions: Vec<KeyValue> = datum.dimensions.into_iter()
                        .filter_map(|(name, dimension)| {
                            dimension.value.map(|value| {
                                KeyValue {
                                    key: name,
                                    value: Some(AnyValue { value: Some(match value {
                                        goodmetrics::dimension::Value::String(s) => any_value::Value::StringValue(s),
                                        goodmetrics::dimension::Value::Number(n) => any_value::Value::IntValue(n as i64),
                                        goodmetrics::dimension::Value::Boolean(b) => any_value::Value::BoolValue(b),
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
                                opentelemetry_metrics::Metric {
                                    // So yeah, this splays all your metrics across a shared namespace because prometheus / otel.
                                    name: format!("{metric_name}_{measurement_name}", metric_name = datum.metric, measurement_name=name),
                                    description: "goodmetrics compatibility conversion".to_string(),
                                    unit: "1".to_string(),
                                    data: Some(match value {
                                        goodmetrics::measurement::Value::I64(i) => opentelemetry_metrics::metric::Data::Gauge(opentelemetry_metrics::Gauge {
                                            data_points: vec![
                                                int_data_point(i, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                        goodmetrics::measurement::Value::I32(i) => opentelemetry_metrics::metric::Data::Gauge(opentelemetry_metrics::Gauge {
                                            data_points: vec![
                                                int_data_point(i as i64, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                        goodmetrics::measurement::Value::F64(f) => opentelemetry_metrics::metric::Data::Gauge(opentelemetry_metrics::Gauge {
                                            data_points: vec![
                                                float_data_point(f, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                        goodmetrics::measurement::Value::F32(f) => opentelemetry_metrics::metric::Data::Gauge(opentelemetry_metrics::Gauge {
                                            data_points: vec![
                                                float_data_point(f as f64, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                        goodmetrics::measurement::Value::StatisticSet(ss) => opentelemetry_metrics::metric::Data::Summary(opentelemetry_metrics::Summary {
                                            data_points: vec![
                                                // Well, this is the closest thing in opentelemetry. Summaries are _terrible_ though because
                                                // they encourage the incredibly error-prone practice of recording quantiles from the source.
                                                summary_data_point(ss, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                        goodmetrics::measurement::Value::Histogram(h) => opentelemetry_metrics::metric::Data::Histogram(opentelemetry_metrics::Histogram {
                                            aggregation_temporality: opentelemetry_metrics::AggregationTemporality::Delta as i32,
                                            data_points: vec![
                                                // Well, this is the closest thing in opentelemetry. Summaries are _terrible_ though because
                                                // they encourage the incredibly error-prone practice of recording quantiles from the source.
                                                histogram_data_point(h, datum.unix_nanos, &dimensions),
                                            ],
                                        }),
                                        goodmetrics::measurement::Value::Tdigest(t) => {
                                            unimplemented!("tdigest for opentelemetry is not supported: {t:?}")
                                        },
                                    }),
                                }
                            })
                        })
                        .collect::<Vec<opentelemetry_metrics::Metric>>()
                })
                .collect();
            match self
                .client
                .export(ExportMetricsServiceRequest {
                    resource_metrics: vec![opentelemetry_metrics::ResourceMetrics {
                        resource: None,
                        schema_url: "".to_string(),
                        instrumentation_library_metrics: vec![
                            opentelemetry_metrics::InstrumentationLibraryMetrics {
                                instrumentation_library: Some(InstrumentationLibrary {
                                    name: "goodmetrics".to_string(),
                                    version: "42".to_string(),
                                }),
                                schema_url: "".to_string(),
                                metrics: export_metrics,
                            },
                        ],
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

fn int_data_point(
    i: i64,
    nano_time: u64,
    dimensions: &[KeyValue],
) -> opentelemetry_metrics::NumberDataPoint {
    opentelemetry_metrics::NumberDataPoint {
        attributes: dimensions.to_owned(),
        start_time_unix_nano: 0,
        time_unix_nano: nano_time,
        exemplars: vec![],
        flags: 0,
        value: Some(opentelemetry_metrics::number_data_point::Value::AsInt(i)),
    }
}

fn float_data_point(
    f: f64,
    nano_time: u64,
    dimensions: &[KeyValue],
) -> opentelemetry_metrics::NumberDataPoint {
    opentelemetry_metrics::NumberDataPoint {
        attributes: dimensions.to_owned(),
        start_time_unix_nano: 0,
        time_unix_nano: nano_time,
        exemplars: vec![],
        flags: 0,
        value: Some(opentelemetry_metrics::number_data_point::Value::AsDouble(f)),
    }
}

fn summary_data_point(
    ss: goodmetrics::StatisticSet,
    nano_time: u64,
    dimensions: &[KeyValue],
) -> opentelemetry_metrics::SummaryDataPoint {
    opentelemetry_metrics::SummaryDataPoint {
        attributes: dimensions.to_owned(),
        start_time_unix_nano: 0,
        time_unix_nano: nano_time,
        flags: 0,
        count: ss.samplecount,
        sum: ss.samplesum,
        quantile_values: vec![
            opentelemetry_metrics::summary_data_point::ValueAtQuantile {
                quantile: 0_f64,
                value: ss.minimum,
            },
            opentelemetry_metrics::summary_data_point::ValueAtQuantile {
                quantile: 1_f64,
                value: ss.maximum,
            },
        ],
    }
}

fn histogram_data_point(
    h: goodmetrics::Histogram,
    nano_time: u64,
    dimensions: &[KeyValue],
) -> opentelemetry_metrics::HistogramDataPoint {
    let buckets: BTreeMap<i64, u64> = h.buckets.into_iter().collect();
    opentelemetry_metrics::HistogramDataPoint {
        attributes: dimensions.to_owned(),
        start_time_unix_nano: 0,
        time_unix_nano: nano_time,
        exemplars: vec![],
        flags: 0,
        count: buckets.values().sum(),

        // Sum is not faithfully maintained in goodmetrics. It's approximate, and over-estimated.
        sum: buckets
            .iter()
            .map(|(bucket, count)| (bucket * *count as i64) as f64)
            .sum(),

        bucket_counts: buckets.values().copied().collect(),
        explicit_bounds: buckets.keys().map(|bucket| *bucket as f64).collect(),
    }
}
