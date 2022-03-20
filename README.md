# <img src="https://user-images.githubusercontent.com/3454741/151748581-1ad6c34c-f583-4813-b878-d19c98ec3427.png" width="108em" align="center"/> Goodmetrics

This is the way (to record metrics)

# Overview
## About
Goodmetrics is for monitoring web service workflows.

Goodmetrics records __observations from workflows__ not __contextless numbers__.

Instead of doing ceremony to conform your workflows to emit interesting data, just record when interesting events
happen at the point of their occurrence and move on. Goodmetrics creates a wide schema for your application workflow, with a column per dimension
and a column per measurement.

### Configurations
**Upstreams**
* Goodmetrics SDK's. If you're a service developer this is where you look.
* Prometheus. If you're stuck with this okay.

**Downstreams**
* TimescaleDB. The good way; with simple, rich and easy to graph wide tables.
* OpenTelemetry otlp. Strips your measurements' relationships to express them as otel types.
  (this is for compatibility, it is not an ideal downstream)

### On healing
Goodmetrics self-heals.

When you have bad data or if someone messes stuff up or whatever - just `drop table problematic_table cascade` and you're good.
If you change a column's data type (illegal) and you didn't change the name, just `alter table problematic_table drop column problematic_column`. It
will recreate with the currently-reported type. Failures to self-heal from missing data are bugs; please report them!

# Data model

## TimescaleDB Direct (the good way)

| Goodmetrics type          | Timescale type | about  |
| :-----:                   | :--:           | ---    |
| `time`                    | timestamptz    | The 1 required column, used as the time column for hypertables. It is provided by Goodmetrics |
| int_dimension             | int8/bigint    | A 64 bit integer |
| str_dimension             | text           | A label |
| bool_dimension            | boolean        | A flag |
| i64                       | int8/bigint    | A 64 bit integer |
| i32                       | int4/int       | A 32 bit integer |
| f64                       | float8         | A 64 bit floating point number |
| f32                       | float4         | A 32 bit floating point number |
| statistic_set_measurement | statistic_set  | A preaggregated {min,max,sum,count} rollup of some value. Has convenience functions for graphing and rollups. |
| histogram_measurement     | histogram      | Implemented as jsonb. Has convenience functions for graphing and rollups. |

## OpenTelemetry (the not-so-good way)

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
* [Kotlin](https://github.com/WarriorOfWire/goodmetrics_kotlin)
* JSON CLI: `goodmetrics` included in this release. `send-metrics` receives json strings.
* Prometheus: `goodmetrics` included in this release. `poll-prometheus` avoid using prometheus when you have other choices.
## JSON CLI
You can shove json into the `goodmetrics` application. You can pass as many Datum blobs in as you want. For example:
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
    "a_histogram":{"value":{"Histogram":{"buckets":{"1":2, "3":4, "5":6}}}}
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
    "a_histogram":{"value":{"Histogram":{"buckets":{"1":2, "3":4, "5":6}}}}
  }
}'
```
Streaming is an option but hasn't seemed like a priority yet.

## Prometheus
If you have some legacy Prometheus component, you can poll it and send data to Timescale. For example, you might emit metrics from a `node_exporter`
& poll it via `goodmetrics poll-prometheus node`. You'd then have tables in Timescale per metric with 1 row per host per metric & dimension position
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
| histogram                | histogram         | These are translated to "sensible histograms" from "bonkers le prometheus histograms" |
| summary                  | f64               | Treated like gauges. These are awful and you shoud never use them if you can possibly use histograms instead |

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

# Philosophy

Not perfect, but it's good.

## Good?
The data model is not bad, like combinatorial metrics engines (think
Prometheus, Cloudwatch, Influx).

It's not perfect, like expensive trace recordings.

It's just good, between these extremes of naivety and high cost.

## Naive huh?
Yup, and I don't mind saying so. Naive counters, gauges and histograms
are fine when you have data that has few dimensions, or when the primary
relationship the data has is intrinsically to time.

When writing a real application for real users, however, you often have
other dimensions that are variously diagnostic, speculative, informative
or accidental (!!). Databases like Influx often find themselves
irreparably polluted by a programming mistake which tagged data with an
unintentional value (ask me how i know :-().

Consider 50 servers, 30 apis (or web pages), 80k users, a geo location
(let's say 15/user), a logged-in/anonymous bit, and 12 measurements per
api. In naive systems, this is modeled as over **40 billion** distinct
series unless you de-relate your data. Go ahead and check the cost of
that cardinality in cloudwatch! You'll have a rough time with any time
series storage engine that stores series the tag-set way. You won't even
be able to make reasonable use of it due to the amount of time the tag
queries take! If you use TimeStream you'll run out of money over rich
service metrics. Do the math on their pricing worksheet and see.

Instead, if you model these interactions as Goodmetrics, you'll have
1 bag of measurements and dimensions per api/web page load. You can also
pre-aggregate the data if you've got some high frequency stuff to record.
You don't have to but it's an option if you don't want a massive database.

Not only that, your data remains related. You can pivot your measurements
by any dimension or value threshold, because you are standing on the shoulders
of decades of great minds investing in PostgresQL. It might sound sexy to
reinvent time series data storage, but for service monitoring... "boring" is
good.

## Not perfect huh?
You could make every single stack in your application a unit of work and emit
a distinct row for it, with invocation IDs so you could join all of the stacks
and view detailed traces. Doing that for every request on a responsive web
application, or on a web application with a high rate of requests puts obvious
pressure on your downstream storage. Some services, like Lightstep, have elaborate
means to collect torrents of traces and sift through them for the interesting ones.

These systems are awesome. They require significant processor and network resources
from your application to construct, track, serialize and emit details about every
API invocation from your system. If you can afford this whether because your rate is
low or your budget is immense, it's a very powerful way to observe a system.

Goodmetrics units of work are more like a trace with only 1 level - and no
intermediate filter server. Goodmetrics can be pre-aggregated on the reporting
server as well, to keep row rate very low and dashboards maximally responsive.

# General
## On white box metrics
While you can handle all of your prometheus-esque counters and gauges,
Goodmetrics is not ideally suited for modeling hierarchical traces.
The intended use case for Goodmetrics is a "unit of work" inside of
your application.

You want Goodmetrics for white box metrics: For recording features of a
workflow (dimensions like `user_id`, `upstream_service` and `upstream_response`)
and observations from that workflow (measurements like `db_latency` and
`request_bytes`).

Remember that your white-box metrics will change. You will need more dimensions
and more measurements down the road. If your series count explodes by a factor
of the cardinality of a new dimension, you may not be able to add a new dimension
down the road. In my experience you tend to learn that _after_ adding that dimension
and having a bad day. With Goodmetrics and Timescale, a poison dimension value
might make a particular time range in your dashboards very hard to use but after
fixing the poison source, it will heal.

## On failure recovery
Goodmetrics are for operational visibility, not for perfect billing or financial
records. It will drop data when there is a problem it can't recover from or when
the database is down. This is to keep the metrics data flow pipeline as close to
normal at all times as possible. Goodmetrics are best-effort. Generally that
effort should work out and the goal should always be to put data where it belongs
but for the sake of availability and performance, perfection is relaxed.

Poison dimension names / unlimited name cardinality is a degenerate failure case
with Goodmetrics, but can be sorted quickly with `drop table poisoned_table` if
you don't want to deal with it more surgically.

Similarly, a full disk condition can be recovered gracefully, in the manner of a
proper ACID database. Contrast this other self-hosted "time series" stores.

In general, if something is wrong - a bad table, a misnamed column or a column
with the wrong data type - you can just delete the offending table or column.
Goodmetrics tries to self-heal all the time. In fact, a newly encountered metrics
table is just a table with a bunch of missing columns that will be healed as the
columns and data types are encountered.

Schema management is a failure recovery feature.

## A white-box unit of work
What a "unit of work" or "workflow" is depends wholly on your application. Some examples:

### A web server
* One unit of work would be an individual invocation of the GET() handler.
  Add your user or header dimensions at the start of the handler and record
  the things you want where they happen. A Metrics object is a cheap blob
  that accumulates all the observations from a workflow's execution.
* Another would be a background job that runs on a timer to refresh a cache
  or do some database tracing.

### A database
* One transaction is a unit of work. It may span several statements.
* One statement is a unit of work.
* One execution of a background job (like vacuum).

### A microcontroller
* One execution of your main loop() (if you're not using a modern async/await uc)
* One execution of a callback for a user button press (obviously not in the ISR
  but rather in the user-mode handler)
* Periodic snapshot of environment measurements (free memory, processor 
  temperature, etc.)
* One sensor reading.
