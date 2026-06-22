use crate::client::Id;

#[derive(Clone, Debug, Default)]
pub struct CreateSimpleIntegrationOutput {
    pub auth_url: Option<String>,
    pub success: bool,
    pub message: String,
    pub tx_id: Option<Id>,
}
