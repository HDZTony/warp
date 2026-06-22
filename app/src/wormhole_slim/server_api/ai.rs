//! Minimal AI client stub for `wormhole-slim` embed builds.

use async_trait::async_trait;

pub use crate::ai::ambient_agents::AgentSource;

#[async_trait]
pub trait AIClient: Send + Sync {}

#[derive(Debug, Default)]
pub struct StubAIClient;

#[async_trait]
impl AIClient for StubAIClient {}
