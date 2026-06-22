//! Minimal team client stub for `wormhole-slim` embed builds.

use async_trait::async_trait;

#[async_trait]
pub trait TeamClient: Send + Sync {}

#[derive(Debug, Default)]
pub struct StubTeamClient;

#[async_trait]
impl TeamClient for StubTeamClient {}
