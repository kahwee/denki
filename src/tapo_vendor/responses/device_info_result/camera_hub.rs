use serde::{Deserialize, Serialize};

use crate::tapo_vendor::error::Error;
use crate::tapo_vendor::responses::{DecodableResultExt, TapoResponseExt};
use crate::tapo_vendor::utils::bool_from_int_or_bool;

/// Device info of Tapo camera hubs (H200, H500).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(any(), pyo3::prelude::pyclass(from_py_object, get_all))]
#[allow(missing_docs)]
pub struct DeviceInfoCameraHubResult {
    pub avatar: String,
    pub bind_status: bool,
    pub child_num: u32,
    #[serde(alias = "dev_id")]
    pub device_id: String,
    pub device_info: String,
    pub device_name: String,
    #[serde(alias = "sw_version")]
    pub fw_ver: String,
    #[serde(deserialize_with = "bool_from_int_or_bool")]
    pub has_set_location_info: bool,
    pub hw_id: String,
    #[serde(alias = "hw_version")]
    pub hw_ver: String,
    #[serde(alias = "local_ip")]
    pub ip: String,
    pub latitude: Option<i64>,
    pub longitude: Option<i64>,
    pub mac: String,
    #[serde(alias = "device_model")]
    pub model: String,
    #[serde(alias = "device_alias")]
    pub nickname: String,
    pub oem_id: String,
    pub product_name: String,
    pub region: Option<String>,
    pub status: String,
    #[serde(alias = "device_type")]
    pub r#type: String,
}

#[cfg(any())]
crate::tapo_vendor::impl_to_dict!(DeviceInfoCameraHubResult);

impl TapoResponseExt for DeviceInfoCameraHubResult {}

impl DecodableResultExt for DeviceInfoCameraHubResult {
    fn decode(self) -> Result<Self, Error> {
        // SmartCam devices don't base64-encode fields.
        Ok(self)
    }
}
