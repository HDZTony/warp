use crate::queries::api_keys::ApiKeyProperties;

#[derive(Clone, Debug)]
pub struct GenerateApiKeyOutput {
    pub raw_api_key: String,
    pub api_key: crate::queries::api_keys::ApiKeyProperties,
}

#[derive(Clone, Debug)]
pub enum GenerateApiKeyResult {
    GenerateApiKeyOutput(GenerateApiKeyOutput),
    UserFacingError(crate::error::UserFacingErrorInterface),
    Unknown,
}

#[derive(Clone, Debug, Default)]
pub struct GenerateApiKeyVariables;
