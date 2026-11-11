use serde::{Deserialize, Serialize};

/// Plug Default State.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
#[cfg_attr(
    any(),
    pyo3::prelude::pyclass(from_py_object, get_all, eq, hash, frozen)
)]
#[allow(missing_docs)]
pub enum DefaultPlugState {
    Custom { state: PlugState },
    LastStates {},
}

/// Plug State.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(
    any(),
    pyo3::prelude::pyclass(from_py_object, get_all, eq, hash, frozen)
)]
#[allow(missing_docs)]
pub struct PlugState {
    pub on: bool,
}

#[cfg(any())]
crate::tapo_vendor::impl_to_dict!(DefaultPlugState);

#[cfg(any())]
crate::tapo_vendor::impl_to_dict!(PlugState);
