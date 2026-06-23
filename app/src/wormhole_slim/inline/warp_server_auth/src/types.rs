//! Shared SaaS scalar types (formerly from Warp GraphQL).

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum OwnerType {
    User,
    Team,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct ServerTimestamp(DateTime<Utc>);

impl ServerTimestamp {
    pub fn new(time: DateTime<Utc>) -> Self {
        Self(time)
    }

    pub fn from_unix_timestamp_micros(ms_since_epoch: i64) -> Result<Self> {
        let date_time = DateTime::from_timestamp_micros(ms_since_epoch)
            .ok_or_else(|| anyhow!("Unable to convert microseconds into NaiveDateTime"))?;
        Ok(Self::new(date_time))
    }

    pub fn timestamp_micros(&self) -> i64 {
        self.0.timestamp_micros()
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        self.0
    }

    pub fn utc(&self) -> DateTime<Utc> {
        self.0
    }
}

impl From<DateTime<Utc>> for ServerTimestamp {
    fn from(value: DateTime<Utc>) -> Self {
        Self::new(value)
    }
}

impl Default for ServerTimestamp {
    fn default() -> Self {
        Self::new(DateTime::default())
    }
}
