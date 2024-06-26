use std::collections::HashMap;

use communication::proto::goodmetrics;
use postgres_types::Type;
use serde_json::json;
use tokio_postgres::{error::SqlState, Client, GenericClient};

use crate::sink::sink_error::SinkError;

use super::postgres_connector::PostgresConnector;

pub async fn get_or_create_histogram_type(
    connector: &mut PostgresConnector,
) -> Result<SpecialTypes, SinkError> {
    let connection = connector.use_connection().await?;
    match get_special_types(connection.client()).await {
        Ok(def) => Ok(def),
        Err(e) => {
            if let Some(dbe) = e.as_db_error() {
                match *dbe.code() {
                    SqlState::UNDEFINED_OBJECT => {
                        log::info!(
                            "Probably missing histogram type. Going to try to make it: {:?}",
                            dbe
                        );
                        drop(connection);

                        let connection = connector.use_connection().await?;
                        let t = create_histogram_type(connection.client()).await?;

                        Ok(t)
                    }
                    _ => {
                        log::info!("Can't find the histogram type, so I can't run: {:?}", dbe);

                        Err(SinkError::Postgres(e))
                    }
                }
            } else {
                Err(SinkError::Postgres(e))
            }
        }
    }
}

pub struct SpecialTypes {
    pub histogram_type: Type,
    pub tdigest_type: Type,
}

async fn get_special_types(client: &Client) -> Result<SpecialTypes, tokio_postgres::Error> {
    match client.prepare("SELECT $1::histogram, $2::tdigest").await {
        Ok(statement) => {
            let histogram_type = statement.params()[0].clone();
            let tdigest_type = statement.params()[1].clone();

            Ok(SpecialTypes {
                histogram_type,
                tdigest_type,
            })
        }
        Err(e) => Err(e),
    }
}

pub fn to_jsonmap(histogram: &goodmetrics::Histogram) -> serde_json::Value {
    let a: HashMap<String, u64> = histogram
        .buckets
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();
    json!(a)
}

async fn create_histogram_type(client: &Client) -> Result<SpecialTypes, tokio_postgres::Error> {
    match client.batch_execute(r#"
-- Data type alias for readability
create domain histogram as jsonb;

-- Shared function to glob up a histogram from a number column
CREATE OR REPLACE FUNCTION histogram_accumulate(internal_state histogram, next_row double precision) RETURNS histogram
AS $fn$
DECLARE
    bucket text;
    floor_log numeric;
BEGIN
    IF next_row = 0
        THEN bucket := '0';
        ELSE
            floor_log := POW(10, FLOOR(LOG(10, next_row::numeric)));
            bucket := (CEIL(next_row * 2 / floor_log) / 2 * floor_log)::text;
    END IF;
    IF internal_state ? bucket
        THEN internal_state := jsonb_set( internal_state, ARRAY[bucket], to_jsonb((internal_state->bucket)::bigint + 1) );
        ELSE internal_state := jsonb_insert( internal_state, ARRAY[bucket], to_jsonb(1) );
    END IF;
    return internal_state;
END;
$fn$ LANGUAGE plpgsql STRICT IMMUTABLE PARALLEL SAFE;

-- Shared function to combine histograms
CREATE OR REPLACE FUNCTION histogram_combine(internal_state histogram, next_row histogram) RETURNS histogram
AS $fn$
DECLARE
    _key   text;
    _value bigint;
BEGIN
    FOR _key, _value IN SELECT * from jsonb_each_text(next_row) LOOP
        IF internal_state ? _key
            THEN internal_state := jsonb_set( internal_state, ARRAY[_key], to_jsonb((internal_state->_key)::bigint + _value) );
            ELSE internal_state := jsonb_insert( internal_state, ARRAY[_key], to_jsonb(_value) );
        END IF;
    END LOOP;
    return internal_state;
END;
$fn$ LANGUAGE plpgsql STRICT IMMUTABLE PARALLEL SAFE;

CREATE OR REPLACE FUNCTION histogram_combine_inv(internal_state histogram, next_row histogram) RETURNS histogram
AS $fn$
DECLARE
    _key   text;
    _value bigint;
BEGIN
    FOR _key, _value IN SELECT * from jsonb_each_text(next_row) LOOP 
        IF internal_state->_key = _value
            THEN internal_state := internal_state - _key;
            ELSE internal_state := jsonb_set( internal_state, ARRAY[_key], to_jsonb((internal_state->_key)::bigint - _value) );
        END IF;
    END LOOP;
    raise notice 'i invertesd';
    return internal_state;
END;
$fn$ LANGUAGE plpgsql STRICT IMMUTABLE PARALLEL SAFE;


-- For downsampling a numeric row into a histogram
CREATE OR REPLACE AGGREGATE accumulate_seh(double precision)
(
    sfunc = histogram_accumulate,
    stype = histogram,
    initcond = '{}',
    combinefunc = histogram_combine,
    PARALLEL = SAFE
);

-- For dynamically combining histograms for presentation
CREATE OR REPLACE AGGREGATE accumulate_seh(histogram)
(
    sfunc = histogram_combine,
    stype = histogram,
    mstype = histogram,
    msfunc = histogram_combine,
    minvfunc = histogram_combine_inv,
    initcond = '{}',
    combinefunc = histogram_combine,
    PARALLEL = SAFE
);


-- Pivots a histogram out to rows of | bucket int | count text | for convenient graphing in Grafana.
-- Used in Grafana like:
-- select
--   time_bucket('1m', time) as time,
--   (buckets(accumulate_seh(some_column))).*  -- Can be numeric or a pre-downsampled SEH column
-- from metrics_table
-- group by 1 order by 1;
CREATE OR REPLACE FUNCTION buckets( seh histogram ) RETURNS TABLE(bucket bigint, count bigint)
AS $$
    select (a.each).key::bigint as bucket, (a.each).value::bigint as count from (select jsonb_each(seh) as each) a;
$$ LANGUAGE SQL STRICT IMMUTABLE PARALLEL SAFE;
    "#).await {
        Ok(_modified) => {
            Ok(get_special_types(client).await?)
        }
        Err(e) => Err(e)
    }
}
