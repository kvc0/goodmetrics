use std::collections::BTreeMap;

use postgres_types::Type;

use crate::proto::goodmetrics::{dimension, measurement, Datum, Dimension, Measurement};

#[derive(Clone)]
pub struct TypeConverter {
    pub statistic_set_type: Type,
    pub histogram_type: Type,
    pub tdigest_type: Type,
}

impl TypeConverter {
    pub fn measurement_sql_type(&self, measurement: &Measurement) -> Option<Type> {
        measurement.value.as_ref().map(|v| match v {
            measurement::Value::I64(_) => Type::INT8,
            measurement::Value::I32(_) => Type::INT4,
            measurement::Value::F64(_) => Type::FLOAT8,
            measurement::Value::F32(_) => Type::FLOAT4,
            measurement::Value::StatisticSet(_) => self.statistic_set_type.clone(),
            measurement::Value::Histogram(_) => Type::JSONB,
            measurement::Value::Tdigest(_) => self.tdigest_type.clone(),
        })
    }

    pub fn dimension_sql_type(&self, dimension: &Dimension) -> Option<Type> {
        dimension.value.as_ref().map(|v| match v {
            dimension::Value::String(_) => Type::TEXT,
            dimension::Value::Number(_) => Type::INT8,
            dimension::Value::Boolean(_) => Type::BOOL,
        })
    }

    pub fn get_dimension_type_map(&self, datums: &[Datum]) -> BTreeMap<String, Type> {
        datums
            .iter()
            .flat_map(|d| d.dimensions.iter())
            .filter_map(|(dimension_name, dimension_value)| {
                self.dimension_sql_type(dimension_value)
                    .map(|sql_type| (dimension_name.clone(), sql_type))
            })
            .collect()
    }

    pub fn get_measurement_type_map(&self, datums: &[Datum]) -> BTreeMap<String, Type> {
        datums
            .iter()
            .flat_map(|d| d.measurements.iter())
            .filter_map(|(measurement_name, measurement_value)| {
                self.measurement_sql_type(measurement_value)
                    .map(|sql_type| (measurement_name.clone(), sql_type))
            })
            .collect()
    }
}
