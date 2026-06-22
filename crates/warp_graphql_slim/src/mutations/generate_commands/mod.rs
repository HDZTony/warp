#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerateCommandsFailureType {
    BadPrompt,
    AiProviderError,
    RateLimited,
    Other,
}

#[derive(Clone, Debug, Default)]
pub struct GeneratedCommandParameter {
    pub id: String,
    pub description: String,
}

#[derive(Clone, Debug, Default)]
pub struct GeneratedCommand {
    pub command: String,
    pub description: String,
    pub parameters: Vec<GeneratedCommandParameter>,
}

#[derive(Clone, Debug, Default)]
pub struct GenerateCommandsResult;

#[derive(Clone, Debug, Default)]
pub struct GenerateCommandsVariables;

impl GenerateCommandsResult {
    pub fn build(_vars: GenerateCommandsVariables) -> crate::client::OperationMarker<Self> {
        crate::client::OperationMarker::new()
    }
}
