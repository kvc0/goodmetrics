# <img src="https://user-images.githubusercontent.com/3454741/151748581-1ad6c34c-f583-4813-b878-d19c98ec3427.png" width="108em" align="center"/> Goodmetrics

Light, fast, unlimited cardinality, service-focused time series metrics.

# Overview
## About
Goodmetrics is for monitoring web service workflows: It records __contextual observations from workflows__ rather than the __contextless numbers__ of other collection systems. (Think "single-level trace" and you're close)

Leveraging [Postgresql and the Timescaledb plugin](https://docs.timescale.com/), Goodmetrics creates a simple, familiar wide schema for your application workflow; a column per dimension and a column per measurement.

## Getting started
### **TimescaleDB**
* https://docs.timescale.com/install/latest/self-hosted/installation-debian/
* Timescale docker
* Timescale Cloud

### Set up timescaledb if you are self-hosting
```sql
create database metrics;
\c metrics
create extension timescaledb;
create extension timescaledb_toolkit;
create role metrics_write;
grant create on database metrics to metrics_write;
create user metrics in group metrics_write;
\password metrics

create role metrics_read;
grant pg_read_all_data to metrics_read;
create user grafana in group metrics_read;
\password grafana
```

### **Run the server**
You can use the latest release's `goodmetricsd` or you can use docker via
```
# Or instead of -p you can --network host
docker run --name goodmetrics -p 9573:9573 --detach kvc0/goodmetrics -- \
  --connection-string 'host=postgres_server_ip_address port=2345 user=metrics password=metrics'
```
### **Send metrics**
Use an SDK or just invoke the latest release's `goodmetrics` cli utility.
Here's an example sending 2 observations of the same metric with a few dimensions and a few different
measurement types:
```
goodmetrics send '
{
  "metric":"test_api",
  "unix_nanos":'`date +%s`'000000000,
  "dimensions":{
    "a_string_dimension":{"value":{"String":"asdf"}},
    "an_integer_dimension":{"value":{"Number":16}},
    "a_boolean_dimension":{"value":{"Boolean":true}}
  },
  "measurements":{
    "an_int_measurement":{"value":{"I32":42}},
    "a_long_measurement":{"value":{"I64":42}},
    "a_float_measurement":{"value":{"F32":42.42}},
    "a_double_measurement":{"value":{"F64":42.42}},
    "a_statistic_set":{"value":{"StatisticSet": {"minimum":1, "maximum":2, "samplesum":8, "samplecount":6}}},
    "a_histogram":{"value":{"Histogram":{"buckets":{"1":2, "3":4, "5":6}}}},
    "a_tdigest":{"value":{"Tdigest":{"sum":42.3,"count":42,"min":1,"max":1.3,"centroids":[{"mean":1,"weight":41},{"mean":1.3,"weight":1}]}}}
  }
} ' '{
  "metric":"test_api",
  "unix_nanos":'`date +%s`'000000000,
  "dimensions":{
    "a_string_dimension":{"value":{"String":"asdf"}},
    "an_integer_dimension":{"value":{"Number":16}},
    "a_boolean_dimension":{"value":{"Boolean":true}}
  },
  "measurements":{
    "an_int_measurement":{"value":{"I32":42}},
    "a_long_measurement":{"value":{"I64":42}},
    "a_float_measurement":{"value":{"F32":42.42}},
    "a_double_measurement":{"value":{"F64":42.42}},
    "a_statistic_set":{"value":{"StatisticSet": {"minimum":1, "maximum":2, "samplesum":8, "samplecount":6}}},
    "a_histogram":{"value":{"Histogram":{"buckets":{"1":2, "3":4, "5":6}}}},
    "a_tdigest":{"value":{"Tdigest":{"sum":42.3,"count":42,"min":1,"max":1.3,"centroids":[{"mean":1,"weight":41},{"mean":1.3,"weight":1}]}}}
  }
}'
```

### Configurations
**Upstreams**
* Goodmetrics SDK's. If you're a service developer this is where to look.
* `goodmetrics` cli. If you're scripting some bach this might be your ticket.
* Prometheus. If you're stuck with this then okay. You can use `goodmetrics` to adapt it.

**Downstreams**
* TimescaleDB. The good way; with simple, rich and easy to graph wide tables.
* OpenTelemetry otlp. Strips your measurements' relationships to express them as otel types.
  This is for compatibility. Most otlp metrics stores will struggle with Goodmetrics cardinality.

### On healing
Goodmetrics self-heals schema, and thinks that data from now is most important.

When you have bad data, `drop table problematic_table cascade` and you're good. If you change a column's data type (illegal) and you didn't change the name, just `alter table problematic_table drop column problematic_column`. It will recreate that column with the currently-reported type.

When there's a problem with data or connections, data gets dropped. Goodmetrics doesn't queue for very long, favoring your service's time to recovery and the _now_ over the nice-to-have of data from time gone by.

# Data model

## TimescaleDB Direct

| Goodmetrics type          | Timescale type | about  |
| :-----:                   | :--:           | ---    |
| `time`                    | timestamptz    | The 1 required column, used as the time column for hypertables. It is provided by goodmetrics |
| int_dimension             | int8/bigint    | A 64 bit integer |
| str_dimension             | text           | A label |
| bool_dimension            | boolean        | A flag |
| i64                       | int8/bigint    | A 64 bit integer |
| i32                       | int4/int       | A 32 bit integer |
| f64                       | float8         | A 64 bit floating point number |
| f32                       | float4         | A 32 bit floating point number |
| statistic_set             | statistic_set  | A preaggregated {min,max,sum,count} rollup of some value. Has convenience functions for graphing and rollups. |
| histogram                 | histogram      | Implemented as jsonb. Has convenience functions for graphing and rollups. |
| t_digest        **[beta]**    | tdigest        | [Fancy](https://github.com/tdunning/t-digest/blob/main/docs/t-digest-paper/histo.pdf) space-constrained and high-speed histogram sketch. Uses timescaledb_toolkit functions for graphing. |

## OpenTelemetry (compatibility)

| Goodmetrics type          | OpenTelemetry Metrics type | about  |
| :-----:                   | :--:                       | ---    |
| `time`                    | time_unix_nano             | Goodmetrics clients timestamp at the start of their workflows by default |
| int_dimension             | Int attribute              | A 64 bit integer |
| str_dimension             | String attribute           | A label |
| bool_dimension            | Bool attribute             | A flag |
| i64                       | Number data point (i64)    | A 64 bit integer |
| i32                       | Number data point (i64)    | OpenTelemetry only represents 64 bit long integers - no 32 bit ints |
| f64                       | Number data point (f64)    | A 64 bit floating point number |
| f32                       | Number data point (f64)    | OpenTelemetry only represents 64 bit double precision - no single precision floats. |
| statistic_set_measurement | Summary data point         | Quantiles 0.0 and 1.0 are populated for min and max. Sum is approximate (over-shoots, computed from buckets). Count is exact. |
| histogram_measurement     | Histogram data point       | Delta temporality only. There is no sense in anything else for services. |

# Clients
* [Rust](https://github.com/kvc0/goodmetrics_rs)
* [Kotlin](https://github.com/kvc0/goodmetrics_kotlin)
* JSON CLI: `goodmetrics` included in this release.
  `send-metrics`: receives json strings.
* Prometheus: `goodmetrics` included in this release.
  `poll-prometheus`: avoid using prometheus when you have other choices.

## JSON CLI
You can shove json into the `goodmetrics` application. You can pass repeated Datum blobs. For example:
```
goodmetrics send '{
  "metric":"mm",
  "unix_nanos":1642367609000000000,
  "dimensions":{
    "a_string_dimension":{"value":{"String":"asdf"}},
    "an_integer_dimension":{"value":{"Number":16}},
    "a_boolean_dimension":{"value":{"Boolean":true}}
  },
  "measurements":{
    "an_int_measurement":{"value":{"I32":42}},
    "a_long_measurement":{"value":{"I64":42}},
    "a_float_measurement":{"value":{"F32":42.42}},
    "a_double_measurement":{"value":{"F64":42.42}},
    "a_statistic_set":{"value":{"StatisticSet": {"minimum":1, "maximum":2, "samplesum":8, "samplecount":6}}},
    "a_histogram":{"value":{"Histogram":{"buckets":{"1":2, "3":4, "5":6}}}},
    "a_tdigest":{"value":{"Tdigest":{"sum":42.3,"count":42,"min":1,"max":1.3,"centroids":[{"mean":1,"weight":41},{"mean":1.3,"weight":1}]}}}
  }
}' '{
  "metric":"mm",
  "unix_nanos":1642367704000000000,
  "dimensions":{
    "a_string_dimension":{"value":{"String":"asdf"}},
    "an_integer_dimension":{"value":{"Number":16}},
    "a_boolean_dimension":{"value":{"Boolean":true}}
  },
  "measurements":{
    "an_int_measurement":{"value":{"I32":42}},
    "a_long_measurement":{"value":{"I64":42}},
    "a_float_measurement":{"value":{"F32":42.42}},
    "a_double_measurement":{"value":{"F64":42.42}},
    "a_statistic_set":{"value":{"StatisticSet": {"minimum":1, "maximum":2, "samplesum":8, "samplecount":6}}},
    "a_histogram":{"value":{"Histogram":{"buckets":{"1":2, "3":4, "5":6}}}},
    "a_tdigest":{"value":{"Tdigest":{"sum":42.3,"count":42,"min":1,"max":1.3,"centroids":[{"mean":1,"weight":41},{"mean":1.3,"weight":1}]}}}
  }
}'
```
Streaming from stdin will happen sooner or later.

## Prometheus
If you have some legacy Prometheus component, you can poll it and send data to Timescale. For example, you might emit metrics from a `node_exporter` & poll it via `goodmetrics poll-prometheus node`. You'd then have tables in Timescale per metric with 1 row per host per metric & dimension position
per poll interval, and a column per dimension/tag.
```
goodmetrics poll-prometheus --help
Poll prometheus metrics

USAGE:
    goodmetrics poll-prometheus [OPTIONS] <prefix> [poll-endpoint]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
        --bonus-dimensions <bonus-dimensions>     [default: {}]
            ex: '{"a_dimension_name": {"value": {"String": "a string dimension value"}} }'
        --interval-seconds <interval-seconds>     [default: 10]

ARGS:
    <prefix>
    <poll-endpoint>     [default: http://127.0.0.1:9100/metrics]
```

### Prometheus -> Goodmetrics type mapping

| Prometheus type          | Goodmetrics type  | about  |
| :-----:                  | :--:              | ---    |
| counter                  | f64               | All counters are treated as f64 |
| gauge                    | f64               | Gauges are just f64 |
| untyped                  | f64               | We just treat untyped like gauge |
| histogram                | histogram         | These are translated to sparse histograms from the bonkers prometheus histograms |
| summary                  | f64               | Treated like gauges. These are awful and you should never use them if you can possibly use histograms instead |

Example grafana query:
```sql
with top_10_by_net_out_bytes as (
  select hostname from node_net_out_bytes where time > now() - interval '3 hours'
  order by value desc
  limit 10
)
select time, hostname, value as cpu_utilization
from node_cpu_utilization
where
  time > now() - interval '3 hours'
  and hostname in (select hostname from top_10_by_net_out_bytes)
order by time
```

## Pictures
![general overview of deployment shape](https://user-images.githubusercontent.com/3454741/153928842-44b5eb0e-74b1-4e48-9f49-0db1d0490e57.svg)

# Development
Both rustfmt and clippy are checked on PR. This repo currently treats all clippy lint violations as errors.

## Add pre-commit hook:
Runs linters on commit to help you check in code that passes PR checks.
```
ln -s ../../git_hooks/pre-commit .git/hooks/pre-commit
```
