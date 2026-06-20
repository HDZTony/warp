#[cfg(test)]
pub mod fake_object_client;
pub mod listener;
#[cfg(test)]
pub mod test_utils;
#[cfg(not(feature = "wormhole-slim"))]
pub mod update_manager;
#[cfg(feature = "wormhole-slim")]
#[path = "../../wormhole_slim/server_update_manager.rs"]
pub mod update_manager;
