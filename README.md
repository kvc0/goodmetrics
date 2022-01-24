# Good Metrics

Not perfect, but it's good.

# Data model

| Column                    | type          | about  |
| :-----:                   | :--:          | ---    |
| `time`                    | timestamptz   | The 1 required column, used as the time column for hypertables. It is provided by Good Metrics |
| int_dimension             | int8/bigint   | A 64 bit integer |
| str_dimension             | text          | A label |
| bool_dimension            | boolean       | A flag |
| gauge_measurement         | float8        | Snapshot of a value |
| statistic_set_measurement | statistic_set | A preaggregated {min,max,sum,count} rollup of some value. Has convenience functions for graphing and rollups. |
| histogram_measurement     | histogram     | Implemented as jsonb. Has convenience functions for graphing and rollups. |

# Philosophy
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
unintentional value.

Consider 50 servers, 30 apis (or web pages), 80k users, a geo location
(let's say 15/user), a logged-in/anonymous bit, and 12 measurements per
api. In naive systems, this is modeled as over **40 billion** distinct
series. That will cost you **over $850 million dollars** per month from
CloudWatch with their public pricing (hell, if you negotiate a 99%
discount it's still **$8.6m**). Also, it doesn't work on influx or
prometheus. They just fall over.

Instead, if you model these interactions as Good Metrics, you'll have
1 bag of measurements and dimensions per api/web page load. That's
`servers * units of work` rows per interval. So if you were rollig up
to 1 second per api/page, you'd have 1500 rows per second; each carrying
5 dimension columns and 12 measurement columns. This is hardly breaking
a sweat for a single moderately sized ec2 instance or Timescale forge/cloud.

Not only that, your data remains related. You can pivot your measurements
by any dimension or value threshold, because you are standing on the shoulders
of decades of great minds investing in SQL. It might sound sexy to reinvent
data storage, but most of the time... "good" is best.

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

Good Metrics units of work are more like a trace with only 1 level - and no
intermediate filter server. Good Metrics can be pre-aggregated on the reporting
server as well, to keep row rate very low and dashboards maximally responsive.
If you can understand what to do with a `map<Name, Dimension|Measurement>` you
can understand what to do with Good Metrics.

# About
## General - White box metrics
While you can handle all of your prometheus-esque counters and gauges,
Good Metrics is not ideally suited for modeling hierarchical traces.
The intended use case for Good Metrics is a "unit of work" inside of
your application.

You want Good Metrics for white box metrics: For recording features of a
workflow (dimensions like `user_id`, `upstream_service` and `upstream_response`)
and observations from that workflow (measurements like `db_latency` and
`request_bytes`).

Remember that your white-box metrics will change. You will need more dimensions
and more measurements down the road. If your series count explodes by a factor
of the cardinality of a new dimension, you may not be able to add a new dimension
down the road. In my experience you tend to learn that _after_ adding that dimension
and having a bad day. With Good Metrics and Timescale, a poison dimension value
might make a particular time range in your dashboards very hard to use but after
fixing the poison source, it will heal.

### On failure recovery
Good Metrics are for operational visibility, not for perfect billing or financial
records. It will drop data when there is a problem it can't recover from or when
the database is down. This is to keep the metrics data flow pipeline as close to
normal at all times as possible. Good Metrics are best-effort. Generally that
effort should work out and the goal should always be to put data where it belongs
but for the sake of availability and performance, perfection is relaxed.

Poison dimension names / unlimited name cardinality is a degenerate failure case
with Good Metrics, but can be sorted quickly with `drop table poisoned_table` if
you don't want to deal with it more surgically.

Similarly, a full disk condition can be recovered gracefully, in the manner of a
proper ACID database. Contrast this other self-hosted "time series" stores.

In general, if something is wrong - a bad table, a misnamed column or a column
with the wrong data type - you can just delete the offending table or column.
Good Metrics tries to self-heal all the time. In fact, a newly encountered metrics
table is just a table with a bunch of missing columns that will be healed as the
columns and data types are encountered.

Schema management is a failure recovery feature.

## A white-box unit of work
What a "unit of work" is depends wholly on your application. Some examples:

### A web server
* One unit of work would be an individual invocation of the GET() handler.
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
