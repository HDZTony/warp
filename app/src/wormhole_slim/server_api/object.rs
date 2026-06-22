//! Re-export slim object client types for the server API object module.

pub use cloud_objects::{AccessLevel, MCPGalleryTemplate};
pub use crate::wormhole_slim::object_client::{
    CloudObjectClient as ObjectClient, GetCloudObjectResponse, GuestIdentifier,
    InitialLoadResponse, StubObjectClient,
};
