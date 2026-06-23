#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenerateMetadataForCommandFailureType {
    BadCommand,
    AiProviderError,
    RateLimited,
    Other,
}

#[derive(Clone, Debug, Default)]
pub struct GenerateMetadataForCommandParameter {
    pub name: String,
    pub description: String,
    pub value: String,
}

#[derive(Clone, Debug, Default)]
pub struct GenerateMetadataForCommandSuccess {
    pub parameterized_command: String,
    pub title: String,
    pub description: String,
    pub parameters: Vec<GenerateMetadataForCommandParameter>,
}

#[derive(Clone, Debug, Default)]
pub struct GenerateMetadataForCommandResult;

#[derive(Clone, Debug, Default)]
pub struct GenerateMetadataForCommandVariables;

impl GenerateMetadataForCommandResult {
    pub fn build(
        _vars: GenerateMetadataForCommandVariables,
    ) -> crate::client::OperationMarker<Self> {
        crate::client::OperationMarker::new()
    }
}
