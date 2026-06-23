pub mod client;
#[cfg(feature = "wormhole-slim")]
pub mod cloud_types;
mod envelope;
mod gcp;
mod manager;
#[cfg(feature = "wormhole-slim")]
pub mod noop_client;
mod secret_value;

pub use client::TaskIdentityToken;
pub use envelope::{UploadKey, init as init_envelope};
pub use gcp::{
    GcpCredentials, GcpFederationConfig, GcpWorkloadIdentityFederationError,
    GcpWorkloadIdentityFederationToken, PrepareGcpCredentialsError,
};
pub use manager::{ActorProvider, ManagedSecretManager};
pub use secret_value::ManagedSecretValue;
