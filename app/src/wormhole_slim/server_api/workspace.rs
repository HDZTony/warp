//! Minimal workspace client stub for `wormhole-slim` embed builds.

use async_trait::async_trait;

#[async_trait]
pub trait WorkspaceClient: Send + Sync {}

#[derive(Debug, Default)]
pub struct StubWorkspaceClient;

#[async_trait]
impl WorkspaceClient for StubWorkspaceClient {}
