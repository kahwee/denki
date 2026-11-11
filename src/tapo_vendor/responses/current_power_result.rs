use serde::{Deserialize, Serialize};

use crate::tapo_vendor::responses::TapoResponseExt;

/// Contains the current power reading of the device.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(any(), pyo3::prelude::pyclass(from_py_object, get_all))]
pub struct CurrentPowerResult {
    /// Current power in Watts (W).
    pub current_power: u64,
}
impl TapoResponseExt for CurrentPowerResult {}

#[cfg(any())]
crate::tapo_vendor::impl_to_dict!(CurrentPowerResult);
