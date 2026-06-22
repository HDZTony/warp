//! Block client stub for `wormhole-slim`.

use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;

use crate::ai::generate_block_title::api::{GenerateBlockTitleRequest, GenerateBlockTitleResponse};
use crate::server::block::{Block, DisplaySetting};

#[cfg_attr(test, automock)]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait BlockClient: 'static + Send + Sync {
    async fn unshare_block(&self, _block_id: String) -> Result<(), anyhow::Error>;
    async fn save_block(
        &self,
        _block: &Block,
        _title: Option<String>,
        _show_prompt: bool,
        _display_setting: DisplaySetting,
    ) -> Result<String, anyhow::Error>;
    async fn blocks_owned_by_user(&self) -> Result<Vec<Block>, anyhow::Error>;
    async fn generate_shared_block_title(
        &self,
        _request: GenerateBlockTitleRequest,
    ) -> Result<GenerateBlockTitleResponse, anyhow::Error>;
}

#[derive(Debug, Default)]
pub struct StubBlockClient;

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl BlockClient for StubBlockClient {
    async fn unshare_block(&self, _block_id: String) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn save_block(
        &self,
        _block: &Block,
        _title: Option<String>,
        _show_prompt: bool,
        _display_setting: DisplaySetting,
    ) -> Result<String, anyhow::Error> {
        Ok(String::new())
    }

    async fn blocks_owned_by_user(&self) -> Result<Vec<Block>, anyhow::Error> {
        Ok(Vec::new())
    }

    async fn generate_shared_block_title(
        &self,
        _request: GenerateBlockTitleRequest,
    ) -> Result<GenerateBlockTitleResponse, anyhow::Error> {
        Ok(GenerateBlockTitleResponse {
            title: String::new(),
        })
    }
}
