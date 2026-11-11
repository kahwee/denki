/// Contains a list of supported alarm types (ringtones) of the device.
/// Useful for debugging only.
#[cfg(any())]
#[derive(Debug, serde::Deserialize)]
pub struct SupportedAlarmTypeListResult {
    /// Available alarm types supported by the play_alarm request
    pub alarm_type_list: Vec<String>,
}

#[cfg(any())]
impl super::TapoResponseExt for SupportedAlarmTypeListResult {}
