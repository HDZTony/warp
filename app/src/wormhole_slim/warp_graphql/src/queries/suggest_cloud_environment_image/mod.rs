use crate::client::Id;
use crate::error::UserFacingErrorInterface;

#[derive(Clone, Debug, Default)]
pub struct SuggestCloudEnvironmentImageAuthRequiredOutput {
    pub auth_url: String,
    pub tx_id: Id,
}

#[derive(Clone, Debug, Default)]
pub struct SuggestCloudEnvironmentImageOutput {
    pub image: String,
    pub needs_custom_image: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug)]
pub enum SuggestCloudEnvironmentImageResult {
    SuggestCloudEnvironmentImageOutput(SuggestCloudEnvironmentImageOutput),
    SuggestCloudEnvironmentImageAuthRequiredOutput(SuggestCloudEnvironmentImageAuthRequiredOutput),
    UserFacingError(UserFacingErrorInterface),
    Unknown,
}
