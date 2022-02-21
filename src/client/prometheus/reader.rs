use std::str::Chars;

use lazy_static::lazy_static;
use regex::Regex;

use crate::metrics::{dimension, measurement, Datum, Dimension, Measurement};

lazy_static! {
    // # TYPE go_memstats_alloc_bytes gauge
    static ref MEASUREMENT_TYPE: Regex = Regex::new(r#"^# TYPE (?P<measurement>[\w_]+) (?P<type>[\w]+)$"#).unwrap();
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

    for line in body.lines() {
        log::trace!("{:?}", line);
        let parse_function = match parse_state {
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
        match parse_function(measurement_name, line, now_nanos) {
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

fn read_summary(measurement_name: &str, line: &str, unix_nanos: u64) -> Option<Datum> {
    // You should not use summaries. They are awful. Fuck prometheus for leading you astray.
    read_a_thing(measurement_name, line, unix_nanos)
}

fn read_histogram(measurement_name: &str, line: &str, unix_nanos: u64) -> Option<Datum> {
    // ideally would report a goodmetrics histogram but prometheus histograms can have
    // cumulative buckets or not. Since I dont really know what kind of histogram this
    // is and a goodmerics histogram is always the non-stupid (that is, bucketed) flavor
    // of histogram, I'm leaving this as a basic list of regular metrics.
    // In case it's not clear by now, fuck prometheus.
    read_a_thing(measurement_name, line, unix_nanos)
}

fn read_counter(measurement_name: &str, line: &str, unix_nanos: u64) -> Option<Datum> {
    read_a_thing(measurement_name, line, unix_nanos)
}

fn read_gauge(measurement_name: &str, line: &str, unix_nanos: u64) -> Option<Datum> {
    read_a_thing(measurement_name, line, unix_nanos)
}

fn read_a_thing(measurement_name: &str, line: &str, unix_nanos: u64) -> Option<Datum> {
    if !line.starts_with(measurement_name) {
        return None;
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
                // why, prometheus people, why... did you forget that json exists?
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
                    // All prometheus tags are strings because fuck prometheus.
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
    // the optional timestamp because fuck prometheus.
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
            value: Some(measurement::Value::Fnumber(number)),
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
