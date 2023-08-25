use std::{collections::HashMap, str::Chars};

use lazy_static::lazy_static;
use regex::Regex;

use crate::proto::goodmetrics::{dimension, measurement, Datum, Dimension, Histogram, Measurement};

lazy_static! {
    // # TYPE go_memstats_alloc_bytes gauge
    static ref MEASUREMENT_TYPE: Regex = Regex::new(r"^# TYPE (?P<measurement>\w+) (?P<type>[\w]+)$").unwrap();
    static ref MEASUREMENT_NAME: Regex = Regex::new(r"^(?P<measurement>\w+)").unwrap();

    // http_request_duration_seconds_bucket{le="0.05"} 24054
    static ref HISTOGRAM_BUCKET: Regex = Regex::new(r#"\{.*le="(?P<le_bucket>[^"])".*\}\s*(?P<count>\d+)"#).unwrap();
}

pub async fn read_prometheus(
    location: &str,
    now_nanos: u64,
    table_prefix: &str,
) -> Result<Vec<Datum>, Box<dyn std::error::Error>> {
    let response = reqwest::get(location).await?.text().await?;
    Ok(decode_prometheus(response, now_nanos, table_prefix))
}

fn decode_prometheus(body: String, now_nanos: u64, table_prefix: &str) -> Vec<Datum> {
    let mut parse_state = ParseState::LookingForType;
    let mut measurement_name: &str = "";
    let mut datums: Vec<Datum> = vec![];
    let mut partial_datum: Option<Datum> = None;

    for line in body.lines() {
        log::trace!("{:?}", line);
        let parse_function: fn(&str, &str, u64, Option<Datum>) -> LineState = match parse_state {
            ParseState::LookingForType => {
                let (pstate, mname) = look_for_type(line);
                measurement_name = mname;
                parse_state = pstate;
                continue;
            }
            ParseState::ReadingGauge => read_gauge,
            ParseState::ReadingCounter => read_counter,
            ParseState::ReadingHistogram => read_histogram,
            ParseState::ReadingSummary => read_summary,
        };
        let line_state = parse_function(measurement_name, line, now_nanos, partial_datum.take());
        if let Some(partial) = line_state.partial_datum {
            partial_datum = Some(partial);
            continue;
        }
        match line_state.complete_datum {
            Some(mut datum) => {
                datum.metric = format!("{}{}", table_prefix, datum.metric);
                log::trace!("datum: {:?}", datum);
                datums.push(datum);
            }
            None => {
                let (pstate, mname) = look_for_type(line);
                measurement_name = mname;
                parse_state = pstate;
            }
        }
    }
    datums
}

struct LineState {
    pub complete_datum: Option<Datum>,
    pub partial_datum: Option<Datum>,
}

fn read_summary(
    measurement_name: &str,
    line: &str,
    unix_nanos: u64,
    _partial: Option<Datum>,
) -> LineState {
    // You should not use summaries. They are awful. Shame on Prometheus for leading you astray.
    LineState {
        complete_datum: read_a_thing(measurement_name, line, unix_nanos),
        partial_datum: None,
    }
}

fn read_counter(
    measurement_name: &str,
    line: &str,
    unix_nanos: u64,
    _partial: Option<Datum>,
) -> LineState {
    LineState {
        complete_datum: read_a_thing(measurement_name, line, unix_nanos),
        partial_datum: None,
    }
}

fn read_gauge(
    measurement_name: &str,
    line: &str,
    unix_nanos: u64,
    _partial: Option<Datum>,
) -> LineState {
    LineState {
        complete_datum: read_a_thing(measurement_name, line, unix_nanos),
        partial_datum: None,
    }
}

fn read_a_thing(measurement_name: &str, line: &str, unix_nanos: u64) -> Option<Datum> {
    match MEASUREMENT_NAME.captures(line) {
        Some(capture) => {
            if capture.name("measurement").unwrap().as_str() != measurement_name {
                return None;
            }
        }
        None => return None,
    }

    // FIXME: Need to make a DatumFactory and pass it through so I can do host dimensions

    let mut datum = Datum {
        metric: measurement_name.to_string(),
        unix_nanos,
        ..Default::default()
    };
    let mut chars = line.chars();

    if chars.nth(measurement_name.len()).unwrap_or(' ') == '{' {
        // Dimensions local to the measurement
        enum TagReadState {
            Name,
            Value,
            Delimiter,
        }
        let mut tagstate = TagReadState::Name;
        let mut tag_name = String::default();
        let mut tag_value = String::default();
        loop {
            let maybe_c = chars.next();
            if maybe_c.is_none() {
                break;
            }
            let c = maybe_c.unwrap();
            match tagstate {
                // why, prometheus people, why... did you forget that json exists? I mean you knew at
                // one point and somehow decided that rolling your own was better?
                // https://www.youtube.com/watch?v=4DzoajMs4DM&t=719s
                // This video is hilarious. A perfect demonstration of poor decision making by a
                // community! "You usually have to parse the entire request body with json[...]"
                // and zero people taught this fellow that ndjson exists. Just astounding that the
                // prometheus line protocol was allowed to get to this point.
                // Rolling one's own ascii protocol in the modern world is not an act that should
                // be undertaken lightly. Json parsers have had tremendous effort placed on speed
                // and efficiency. Protocol buffers <exists>. Choosing an alternative like this
                // prometheus text line protocol is the worst possible choice in this age.
                TagReadState::Name => {
                    if c == '=' {
                        chars.next(); // Discard leading "
                        tagstate = TagReadState::Value;
                    } else if c == '}' {
                        chars.next(); // Discard trailing " "
                        break;
                    } else {
                        push_escaped(c, &mut tag_name, &mut chars);
                    }
                }
                TagReadState::Value => {
                    // All prometheus tags are strings because what else could you ever possibly want...
                    if c == '"' {
                        datum.dimensions.insert(
                            tag_name,
                            Dimension {
                                value: Some(dimension::Value::String(tag_value)),
                            },
                        );
                        tag_name = String::default();
                        tag_value = String::default();
                        tagstate = TagReadState::Delimiter;
                    } else {
                        push_escaped(c, &mut tag_value, &mut chars);
                    }
                }
                TagReadState::Delimiter => {
                    if c == ',' {
                        tagstate = TagReadState::Name;
                    } else if c == '}' {
                        chars.next(); // Discard trailing " "
                        break;
                    }
                }
            }
        }
    }
    // Here, we've named the datum and put whatever dimensions it has into it.
    // All that's left is the value and an optional timestamp but I don't use
    // the optional timestamp because.
    let number_string: String = chars.take_while(|c| *c != ' ').collect();
    let number: f64 = number_string.parse().unwrap_or_else(|err| {
        log::error!("bad number format in line. Error: {}, line: {}", err, line);
        -1.0
    });
    // Measurements can be repeated with any tags, so we emit 1 row per dimension position. Idk what else
    // to call the value column... there's not a good choice here that's obvious to me. Prometheus metrics
    // are ass though so yeah sorry about this & I hope you're not stuck with them for important stuff.
    datum.measurements.insert(
        "value".to_string(),
        Measurement {
            value: Some(measurement::Value::F64(number)),
        },
    );

    Some(datum)
}

fn push_escaped(c: char, target: &mut String, chars: &mut Chars) {
    if c == '\\' {
        target.push(c);
        target.push(chars.next().unwrap_or('\\'));
    } else {
        target.push(c);
    }
}

fn look_for_type(line: &str) -> (ParseState, &str) {
    match MEASUREMENT_TYPE.captures(line) {
        Some(capture) => {
            let measurement_name = capture.name("measurement").unwrap().as_str();
            let metric_type = capture.name("type").unwrap().as_str();
            (
                next_parse_state(metric_type, measurement_name),
                measurement_name,
            )
        }
        None => {
            log::debug!("skipping line: {}", line);
            (ParseState::LookingForType, "")
        }
    }
}

fn next_parse_state(metric_type: &str, measurement_name: &str) -> ParseState {
    match metric_type {
        "gauge" => {
            log::trace!("found metric type {}: gauge", measurement_name);
            ParseState::ReadingGauge
        }
        "counter" => {
            log::trace!("found metric type {}: counter", measurement_name);
            ParseState::ReadingCounter
        }
        "histogram" => {
            log::trace!("found metric type {}: histogram", measurement_name);
            ParseState::ReadingHistogram
        }
        "untyped" => {
            log::trace!(
                "found metric type {}: untyped - treating as gauge",
                measurement_name
            );
            ParseState::ReadingGauge
        }
        "summary" => {
            log::trace!("found metric type {}: histogram", measurement_name);
            ParseState::ReadingSummary
        }
        _ => {
            log::warn!(
                "unknown metric type {}: {} - skipping",
                measurement_name,
                metric_type
            );
            ParseState::LookingForType
        }
    }
}

enum ParseState {
    LookingForType,
    ReadingGauge,
    ReadingCounter,
    ReadingHistogram,
    ReadingSummary,
}

// here's the bs histogram format; i have no idea if it is supposed to support dimensions other than le,
// which is of course a terrible way to model a histogram. This format was a spectacularly bad idea.
// # TYPE http_request_duration_seconds histogram
// http_request_duration_seconds_bucket{le="0.05"} 24054
// http_request_duration_seconds_bucket{le="0.1"} 33444
// http_request_duration_seconds_bucket{le="0.2"} 100392
// http_request_duration_seconds_bucket{le="0.5"} 129389
// http_request_duration_seconds_bucket{le="1"} 133988
// http_request_duration_seconds_bucket{le="+Inf"} 144320
// http_request_duration_seconds_sum 53423
// http_request_duration_seconds_count 144320
fn read_histogram(
    measurement_name: &str,
    line: &str,
    unix_nanos: u64,
    partial: Option<Datum>,
) -> LineState {
    let mut datum = match partial {
        Some(p) => p,
        None => Datum {
            metric: measurement_name.to_string(),
            unix_nanos,
            measurements: HashMap::from([(
                "value".to_string(),
                Measurement {
                    value: Some(measurement::Value::Histogram(Histogram {
                        buckets: HashMap::new(),
                    })),
                },
            )]),
            ..Default::default()
        },
    };
    return if line.starts_with(&format!("{}_bucket{{", measurement_name)) {
        let v = datum.measurements.get_mut("value");
        let histogram = match v.unwrap().value.as_mut().unwrap() {
            measurement::Value::Histogram(h) => h,
            _ => {
                log::error!("Bad histogram type in datum {:?}", datum);
                return LineState {
                    complete_datum: None,
                    partial_datum: None,
                };
            }
        };
        match HISTOGRAM_BUCKET.captures(line) {
            Some(capture) => {
                let bucket_str = match capture.name("le_bucket") {
                    Some(b) => b.as_str(),
                    None => {
                        return LineState {
                            complete_datum: None,
                            partial_datum: None,
                        }
                    }
                };
                let raw_count: u64 = match capture.name("count") {
                    Some(c) => c.as_str().parse::<u64>().unwrap_or(0),
                    None => {
                        return LineState {
                            complete_datum: None,
                            partial_datum: None,
                        }
                    }
                };
                let raw_bucket: f64 = match bucket_str.parse() {
                    Ok(b) => b,
                    Err(e) => {
                        log::error!("bad histogram bucket line: {}, {:?}", line, e);
                        return LineState {
                            complete_datum: None,
                            partial_datum: None,
                        };
                    }
                };
                let values_below_bucket: u64 = histogram
                    .buckets
                    .iter()
                    .map(|(k, v)| if (*k as f64) < raw_bucket { *v } else { 0 })
                    .sum();

                let actual_count = raw_count - values_below_bucket;
                let actual_bucket = raw_bucket.ceil() as i64;
                let bucket_value = histogram
                    .buckets
                    .remove(&actual_bucket)
                    .map_or(actual_count, |v| v + actual_count);
                histogram.buckets.insert(actual_bucket, bucket_value);
                LineState {
                    complete_datum: None,
                    partial_datum: Some(datum),
                }
            }
            None => LineState {
                complete_datum: None,
                partial_datum: None,
            },
        }
    } else {
        // if line.starts_with(&format!("{}_sum", measurement_name))
        // then we'll guess that it's the sum thing from the histogram.
        // goodmetrics histograms are for distributions though, so we
        // don't really care about the raw sum.

        // if line.starts_with(&format!("{}_count", measurement_name))
        // i mean, the histogram already has the raw count... c'mon prometheus...

        // else
        // Maybe there was no sum or count? I dunno, the line protocol is not
        // well-specified.
        LineState {
            complete_datum: Some(datum),
            partial_datum: None,
        }
    };
}
