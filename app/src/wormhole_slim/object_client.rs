//! Minimal cloud-object client stubs for `wormhole-slim` embed builds.

use async_trait::async_trait;
pub use cloud_object_client::{
    GetCloudObjectResponse, GuestIdentifier, InitialLoadResponse, ObjectClient,
};
pub use cloud_object_client::ObjectClient as CloudObjectClient;

#[derive(Debug, Default)]
pub struct StubObjectClient;

/// Marker trait retained for slim [`UpdateManager`] wiring.
#[async_trait]
pub trait SlimObjectClient: Send + Sync {}

#[async_trait]
impl SlimObjectClient for StubObjectClient {}

#[path = "object_client_stub_impl.rs"]
mod object_client_stub_impl;
