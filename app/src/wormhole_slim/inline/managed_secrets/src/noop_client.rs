use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;

use crate::client::{
    IdentityTokenOptions, ManagedSecretConfigs, ManagedSecretsClient, SecretOwner,
    TaskIdentityToken,
};

#[cfg(feature = "wormhole-slim")]
use crate::cloud_types::{AgentHarness, ManagedSecret, ManagedSecretType, TaskManagedSecretValue};

#[cfg(not(feature = "wormhole-slim"))]
use warp_graphql::managed_secrets::{ManagedSecret, ManagedSecretType};
#[cfg(not(feature = "wormhole-slim"))]
use warp_graphql::queries::task_secrets::ManagedSecretValue as TaskManagedSecretValue;
#[cfg(not(feature = "wormhole-slim"))]
use warp_graphql::ai::AgentHarness;

/// Cloud-disabled managed secrets client for `wormhole-slim` embed builds.
#[derive(Debug, Default)]
pub struct NoopManagedSecretsClient;

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl ManagedSecretsClient for NoopManagedSecretsClient {
    async fn get_managed_secret_configs(&self) -> Result<ManagedSecretConfigs> {
        Ok(ManagedSecretConfigs {
            user_secrets: None,
            team_secrets: HashMap::new(),
        })
    }

    async fn create_managed_secret(
        &self,
        _owner: SecretOwner,
        _name: String,
        _secret_type: ManagedSecretType,
        _encrypted_value: String,
        _description: Option<String>,
    ) -> Result<ManagedSecret> {
        Err(anyhow!("managed secrets are disabled in wormhole-slim builds"))
    }

    async fn delete_managed_secret(&self, _owner: SecretOwner, _name: String) -> Result<()> {
        Err(anyhow!("managed secrets are disabled in wormhole-slim builds"))
    }

    async fn update_managed_secret(
        &self,
        _owner: SecretOwner,
        _name: String,
        _encrypted_value: Option<String>,
        _description: Option<String>,
    ) -> Result<ManagedSecret> {
        Err(anyhow!("managed secrets are disabled in wormhole-slim builds"))
    }

    async fn list_secrets(&self) -> Result<Vec<ManagedSecret>> {
        Ok(Vec::new())
    }

    async fn list_harness_auth_secrets(
        &self,
        _harness: AgentHarness,
    ) -> Result<Vec<ManagedSecret>> {
        Ok(Vec::new())
    }

    async fn get_task_secrets(
        &self,
        _task_id: String,
        _workload_token: String,
    ) -> Result<HashMap<String, TaskManagedSecretValue>> {
        Ok(HashMap::new())
    }

    async fn issue_task_identity_token(
        &self,
        _options: IdentityTokenOptions,
    ) -> Result<TaskIdentityToken> {
        Err(anyhow!("managed secrets are disabled in wormhole-slim builds"))
    }
}
