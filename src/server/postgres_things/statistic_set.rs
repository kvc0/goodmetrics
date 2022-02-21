use postgres_types::Type;
use tokio_postgres::{error::SqlState, Client, GenericClient};

use crate::sink::postgres_sink::SinkError;

use super::postgres_connector::PostgresConnector;

pub async fn get_or_create_statistic_set_type(
    connector: &mut PostgresConnector,
) -> Result<Type, SinkError> {
    let connection = connector.use_connection().await?;
    match get_statistic_set_type(connection.client()).await {
        Ok(def) => Ok(def),
        Err(e) => {
            if let Some(dbe) = e.as_db_error() {
                match *dbe.code() {
                    SqlState::UNDEFINED_OBJECT => {
                        log::info!(
                            "Probably missing statistic_set type. Going to try to make it: {:?}",
                            dbe
                        );
                        drop(connection);

                        let connection = connector.use_connection().await?;
                        let t = create_statistic_set_type(connection.client()).await?;

                        Ok(t)
                    }
                    _ => {
                        log::info!(
                            "Can't find the statistic_set type, so I can't run: {:?}",
                            dbe
                        );

                        Err(SinkError::Postgres(e))
                    }
                }
            } else {
                Err(SinkError::Postgres(e))
            }
        }
    }
}

async fn get_statistic_set_type(client: &Client) -> Result<Type, tokio_postgres::Error> {
    match client.prepare("SELECT $1::statistic_set").await {
        Ok(statement) => {
            let statistic_set_type = statement.params()[0].clone();

            Ok(statistic_set_type)
        }
        Err(e) => Err(e),
    }
}

async fn create_statistic_set_type(client: &Client) -> Result<Type, tokio_postgres::Error> {
    match client.batch_execute(r#"
-- Here is a data type I find highly useful when recording high frequency events:
CREATE TYPE statistic_set AS (
  minimum     double precision,
  maximum     double precision,
  samplesum   double precision,
  samplecount int8
);

--------------------- statistic_set support functions ---------------------
 
-- Shared function to glob up statistic_set for min, max, avg, sum and count
CREATE OR REPLACE FUNCTION statistic_set_accum(internal_state statistic_set, next_row statistic_set) RETURNS statistic_set
AS $$
DECLARE
BEGIN
    IF next_row.minimum < internal_state.minimum THEN internal_state.minimum := next_row.minimum; END IF;
    IF next_row.maximum > internal_state.maximum THEN internal_state.maximum := next_row.maximum; END IF;
    internal_state.samplesum := internal_state.samplesum + next_row.samplesum;
    internal_state.samplecount := internal_state.samplecount + next_row.samplecount;
    return internal_state;
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;
 
-- Shared function to glob up numbers as statistic_set
CREATE OR REPLACE FUNCTION statistic_set_accum(internal_state statistic_set, next_row double precision) RETURNS statistic_set
AS $$
DECLARE
BEGIN
    IF next_row < internal_state.minimum THEN internal_state.minimum := next_row; END IF;
    IF next_row > internal_state.maximum THEN internal_state.maximum := next_row; END IF;
    internal_state.samplesum := internal_state.samplesum + next_row;
    internal_state.samplecount := internal_state.samplecount + 1;
    return internal_state;
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;
 
-- Finalizes an accumulated statistic_set as an average
CREATE OR REPLACE FUNCTION statistic_set_avg(value statistic_set) RETURNS double precision
AS $$
DECLARE BEGIN
    IF value.samplecount = 0 THEN return 0; END IF;
    return value.samplesum / value.samplecount;
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;
 
-- Finalizes an accumulated statistic_set as a min
CREATE OR REPLACE FUNCTION statistic_set_min(value statistic_set) RETURNS double precision
AS $$
DECLARE BEGIN
    return value.minimum;
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;
 
-- Finalizes an accumulated statistic_set as a max
CREATE OR REPLACE FUNCTION statistic_set_max(value statistic_set) RETURNS double precision
AS $$
DECLARE BEGIN
    return value.maximum;
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;
 
-- Finalizes an accumulated statistic_set as a sum
CREATE OR REPLACE FUNCTION statistic_set_sum(value statistic_set) RETURNS double precision
AS $$
DECLARE BEGIN
    return value.samplesum;
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;
 
-- Finalizes an accumulated statistic_set as a count
CREATE OR REPLACE FUNCTION statistic_set_count(value statistic_set) RETURNS double precision
AS $$
DECLARE BEGIN
    return value.samplecount;
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT;

-- avg(column) is equivalent to sum((column).samplesum) / sum((column).samplecount)
CREATE AGGREGATE avg (statistic_set)
(
    sfunc = statistic_set_accum,
    stype = statistic_set,
    finalfunc = statistic_set_avg,
    initcond = '(1E+308,-1E+308,0,0)',
    combinefunc = statistic_set_accum,
    PARALLEL = SAFE
);

-- min(column) is equivalent to min((column).minimum)
CREATE AGGREGATE min (statistic_set)
(
    sfunc = statistic_set_accum,
    stype = statistic_set,
    finalfunc = statistic_set_min,
    initcond = '(1E+308,-1E+308,0,0)',
    combinefunc = statistic_set_accum,
    PARALLEL = SAFE
);

-- max(column) is equivalent to max((column).maximum)
CREATE AGGREGATE max (statistic_set)
(
    sfunc = statistic_set_accum,
    stype = statistic_set,
    finalfunc = statistic_set_max,
    initcond = '(1E+308,-1E+308,0,0)',
    combinefunc = statistic_set_accum,
    PARALLEL = SAFE
);

-- sum(column) is equivalent to sum((column).samplesum)
CREATE AGGREGATE sum (statistic_set)
(
    sfunc = statistic_set_accum,
    stype = statistic_set,
    finalfunc = statistic_set_sum,
    initcond = '(1E+308,-1E+308,0,0)',
    combinefunc = statistic_set_accum,
    PARALLEL = SAFE
);

-- count(column) is equivalent to sum((column).samplecount)
CREATE AGGREGATE count (statistic_set)
(
    sfunc = statistic_set_accum,
    stype = statistic_set,
    finalfunc = statistic_set_count,
    initcond = '(1E+308,-1E+308,0,0)',
    combinefunc = statistic_set_accum,
    PARALLEL = SAFE
);

CREATE AGGREGATE accumulate(statistic_set)
(
    sfunc = statistic_set_accum,
    stype = statistic_set,
    initcond = '(1E+308,-1E+308,0,0)',
    combinefunc = statistic_set_accum,
    PARALLEL = SAFE
);

CREATE AGGREGATE accumulate(double precision)
(
    sfunc = statistic_set_accum,
    stype = statistic_set,
    initcond = '(1E+308,-1E+308,0,0)',
    combinefunc = statistic_set_accum,
    PARALLEL = SAFE
);
    "#).await {
        Ok(_modified) => {
            Ok(get_statistic_set_type(client).await?)
        }
        Err(e) => Err(e)
    }
}
