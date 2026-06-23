#[derive(Clone, Debug, Default)]
pub struct AgentHarnessInput {}

#[derive(Clone, Debug)]
pub enum HarnessAuthSecretsResult {
    Unknown,
    UserFacingError(crate::error::UserFacingErrorInterface),
}

