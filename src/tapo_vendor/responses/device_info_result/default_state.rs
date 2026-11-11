use serde::{Deserialize, Serialize};

/// The type of the default state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    any(),
    pyo3::prelude::pyclass(from_py_object, get_all, eq, hash, frozen)
)]
#[allow(missing_docs)]
pub enum DefaultStateType {
    Custom,
    LastStates,
}

/// The type of the default power state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(
    any(),
    pyo3::prelude::pyclass(from_py_object, get_all, eq, hash, frozen)
)]
#[allow(missing_docs)]
pub enum DefaultPowerType {
    AlwaysOn,
    LastStates,
}

/// Default brightness state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(any(), pyo3::prelude::pyclass(from_py_object, get_all))]
#[allow(missing_docs)]
pub struct DefaultBrightnessState {
    pub r#type: DefaultStateType,
    pub value: u8,
}

#[cfg(any())]
crate::tapo_vendor::impl_to_dict!(DefaultBrightnessState);
