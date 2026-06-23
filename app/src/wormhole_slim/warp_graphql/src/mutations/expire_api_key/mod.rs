#[derive(Clone, Debug)]
pub struct ExpireApiKeyOutput {
    pub success: bool,
}

#[derive(Clone, Debug)]
pub enum ExpireApiKeyResult {
    ExpireApiKeyOutput(ExpireApiKeyOutput),
    UserFacingError(crate::error::UserFacingErrorInterface),
    Unknown,
}

#[derive(Clone, Debug, Default)]
pub struct ExpireApiKeyVariables;
