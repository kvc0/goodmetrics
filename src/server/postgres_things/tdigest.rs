use std::fmt::Display;

use itertools::Itertools;

use crate::proto;

#[derive(Debug)]
pub struct SqlTdigest {
    pub version: u8, //(version:1,buckets:1,max_buckets:10,count:1,sum:42,min:42,max:42,centroids:[(mean:42,weight:1)])
    pub max_buckets: u32,
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    pub centroids: Vec<SqlCentroid>,
}
#[derive(Debug)]
pub struct SqlCentroid {
    pub mean: f64,
    pub weight: u64,
}

impl Display for SqlTdigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "(version:1,max_buckets:{},count:{},sum:{},min:{},max:{},centroids:[{}])",
            self.max_buckets,
            self.count,
            self.sum,
            self.min,
            self.max,
            self.centroids
                .iter()
                .map(|c| format!("(mean:{},weight:{})", c.mean, c.weight))
                .join(",")
        ))
    }
}

impl From<&proto::goodmetrics::TDigest> for SqlTdigest {
    fn from(value: &proto::goodmetrics::TDigest) -> Self {
        Self {
            version: 1,
            max_buckets: 100,
            count: value.count,
            sum: value.sum,
            min: value.min,
            max: value.max,
            centroids: value.centroids.iter().map_into().collect(),
        }
    }
}

impl From<&proto::goodmetrics::t_digest::Centroid> for SqlCentroid {
    fn from(value: &proto::goodmetrics::t_digest::Centroid) -> Self {
        Self {
            mean: value.mean,
            weight: value.weight,
        }
    }
}
