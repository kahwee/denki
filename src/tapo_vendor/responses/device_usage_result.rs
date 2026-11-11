use serde::{Deserialize, Serialize};

use crate::tapo_vendor::responses::TapoResponseExt;
use crate::tapo_vendor::utils::ok_or_default;

/// Contains the time usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(any(), pyo3::prelude::pyclass(from_py_object, get_all))]
pub struct DeviceUsageResult {
    /// Time usage in minutes.
    pub time_usage: UsageByPeriodResult,
}
impl TapoResponseExt for DeviceUsageResult {}

#[cfg(any())]
crate::tapo_vendor::impl_to_dict!(DeviceUsageResult);

/// Usage by period result for today, the past 7 days, and the past 30 days.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(any(), pyo3::prelude::pyclass(from_py_object, get_all))]
pub struct UsageByPeriodResult {
    /// Today usage in minutes.
    #[serde(deserialize_with = "ok_or_default")]
    pub today: Option<u64>,
    /// Past 7 days usage in minutes.
    #[serde(deserialize_with = "ok_or_default")]
    pub past7: Option<u64>,
    /// Past 30 days usage in minutes.
    #[serde(deserialize_with = "ok_or_default")]
    pub past30: Option<u64>,
}

#[cfg(any())]
crate::tapo_vendor::impl_to_dict!(UsageByPeriodResult);
