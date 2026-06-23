#[derive(Clone, Debug, Default)]
pub struct UpdateUserSettingsInput {
    pub cloud_conversation_storage_enabled: Option<bool>,
    pub crash_reporting_enabled: Option<bool>,
    pub telemetry_enabled: Option<bool>,
}

pub struct UpdateUserSettingsOutput;

#[derive(Clone, Debug)]
pub enum UpdateUserSettingsResult {
    Success,
    Unknown,
}
