use crate::scalars::Time;

#[derive(Clone, Debug, Default)]
pub struct ListedSimpleIntegrationConfig {
    pub environment_uid: String,
    pub model_id: String,
    pub base_prompt: String,
    pub mcp_servers_json: String,
}

#[derive(Clone, Debug, Default)]
pub struct SimpleIntegration {
    pub provider_slug: String,
    pub description: String,
    pub connection_status: SimpleIntegrationConnectionStatus,
    pub integration_config: Option<ListedSimpleIntegrationConfig>,
    pub created_at: Option<Time>,
    pub updated_at: Option<Time>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SimpleIntegrationConnectionStatus {
    #[default]
    Unknown,
    NotConnected,
    ConnectionError,
    IntegrationNotConfigured,
    NotEnabled,
    Active,
}

#[derive(Clone, Debug, Default)]
pub struct SimpleIntegrationsOutput {
    pub message: Option<String>,
    pub integrations: Vec<SimpleIntegration>,
}
