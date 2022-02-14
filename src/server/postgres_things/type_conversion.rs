use std::collections::BTreeMap;

use postgres_types::Type;

use crate::proto::metrics::pb::{dimension, measurement, Datum, Dimension, Measurement};

pub struct TypeConverter {
    pub statistic_set_type: Type,
    pub histogram_type: Type,
}

impl TypeConverter {
    pub fn measurement_sql_type(&self, measurement: &Measurement) -> Option<Type> {
        match measurement.value.as_ref() {
            Some(v) => Some(match v {
                measurement::Value::Inumber(_) => Type::INT8,
                measurement::Value::Fnumber(_) => Type::FLOAT8,
                measurement::Value::StatisticSet(_) => self.statistic_set_type.clone(),
                measurement::Value::Histogram(_) => Type::JSONB,
            }),
            None => None,
        }
    }

    pub fn dimension_sql_type(&self, dimension: &Dimension) -> Option<Type> {
        match dimension.value.as_ref() {
            Some(v) => Some(match v {
                dimension::Value::String(_) => Type::TEXT,
                dimension::Value::Number(_) => Type::INT8,
                dimension::Value::Boolean(_) => Type::BOOL,
            }),
            None => None,
        }
    }

    pub fn get_dimension_type_map(&self, datums: &Vec<&Datum>) -> BTreeMap<String, Type> {
        datums
            .iter()
            .map(|d| d.dimensions.iter())
            .flatten()
            .filter_map(|(dimension_name, dimension_value)| {
                if let Some(sql_type) = self.dimension_sql_type(dimension_value) {
                    Some((dimension_name.clone(), sql_type))
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn get_measurement_type_map(&self, datums: &Vec<&Datum>) -> BTreeMap<String, Type> {
        datums
            .iter()
            .map(|d| d.measurements.iter())
            .flatten()
            .filter_map(|(measurement_name, measurement_value)| {
                if let Some(sql_type) = self.measurement_sql_type(measurement_value) {
                    Some((measurement_name.clone(), sql_type))
                } else {
                    None
                }
            })
            .collect()
    }
}
