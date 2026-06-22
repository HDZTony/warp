use super::StubObjectClient;
use anyhow::Result;
use async_channel::Sender;
use chrono::{DateTime, Utc};
use cloud_object_client::*;
use cloud_objects::drive::sharing::SharingAccessLevel;
use cloud_objects::ids::{FolderId, GenericStringObjectId, ServerId};
use cloud_objects::{AccessLevel, MCPGalleryTemplate};
use std::collections::HashMap;

#[cfg_attr(not(target_family = "wasm"), async_trait::async_trait)]
#[cfg_attr(target_family = "wasm", async_trait::async_trait(?Send))]
impl ObjectClient for StubObjectClient {
    async fn create_workflow(&self, request: CreateObjectRequest) -> Result<CreateCloudObjectResult> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn update_workflow(&self, workflow_id: WorkflowId, data: SerializedModel, revision: Option<Revision>) -> Result<UpdateCloudObjectResult<ServerWorkflow>> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn bulk_create_generic_string_objects(&self, owner: Owner, objects: &[BulkCreateGenericStringObjectsRequest]) -> Result<BulkCreateCloudObjectResult> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn create_generic_string_object(&self, format: GenericStringObjectFormat, uniqueness_key: Option<GenericStringObjectUniqueKey>, request: CreateObjectRequest) -> Result<CreateCloudObjectResult> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn create_notebook(&self, request: CreateObjectRequest) -> Result<CreateCloudObjectResult> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn update_notebook(&self, notebook_id: cloud_object_models::NotebookId, title: Option<String>, data: Option<SerializedModel>, revision: Option<Revision>) -> Result<UpdateCloudObjectResult<ServerNotebook>> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn create_folder(&self, request: CreateObjectRequest) -> Result<CreateCloudObjectResult> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn update_folder(&self, folder_id: FolderId, name: SerializedModel) -> Result<UpdateCloudObjectResult<ServerFolder>> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn update_generic_string_object(&self, object_id: GenericStringObjectId, model: SerializedModel, revision: Option<Revision>) -> Result<UpdateCloudObjectResult<Box<dyn ServerObject>>> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn grab_notebook_edit_access(&self, notebook_id: cloud_object_models::NotebookId) -> Result<ServerMetadata> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn give_up_notebook_edit_access(&self, notebook_id: cloud_object_models::NotebookId) -> Result<ServerMetadata> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn get_warp_drive_updates(&self, message_sender: Sender<ObjectUpdateMessage>, stream_ready_sender: Sender<()>) -> Result<()> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn fetch_changed_objects(&self, objects_to_update: ObjectsToUpdate, force_refresh: bool) -> Result<InitialLoadResponse> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn fetch_single_cloud_object(&self, id: ServerId) -> Result<GetCloudObjectResponse> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn transfer_notebook_owner(&self, notebook_id: cloud_object_models::NotebookId, owner: Owner) -> Result<bool> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn transfer_workflow_owner(&self, workflow_id: WorkflowId, owner: Owner) -> Result<bool> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn transfer_generic_string_object_owner(&self, workflow_id: GenericStringObjectId, owner: Owner) -> Result<bool> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn trash_object(&self, id: ServerId) -> Result<bool> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn untrash_object(&self, id: ServerId) -> Result<ObjectMetadataUpdateResult> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn delete_object(&self, id: ServerId) -> Result<ObjectDeleteResult> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn empty_trash(&self, owner: Owner) -> Result<ObjectDeleteResult> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn move_object(&self, id: ServerId, folder_id: Option<FolderId>, owner: Owner, object_type: ObjectType) -> Result<bool> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn record_object_action(&self, id: ServerId, action_type: ObjectActionType, timestamp: DateTime<Utc>, data: Option<String>) -> Result<ObjectActionHistory> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn leave_object(&self, id: ServerId) -> Result<ObjectDeleteResult> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn set_object_link_permissions(&self, object_id: ServerId, access_level: SharingAccessLevel) -> Result<ObjectPermissionUpdateResult> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn remove_object_link_permissions(&self, object_id: ServerId) -> Result<ObjectPermissionUpdateResult> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn add_object_guests(&self, object_id: ServerId, guest_emails: Vec<String>, access_level: AccessLevel) -> Result<ObjectPermissionsUpdateData> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn update_object_guests(&self, object_id: ServerId, guest_emails: Vec<String>, access_level: AccessLevel) -> Result<ServerPermissions> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn remove_object_guest(&self, object_id: ServerId, guest: GuestIdentifier) -> Result<ServerPermissions> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
    async fn fetch_environment_last_task_run_timestamps(&self, ) -> Result<HashMap<String, DateTime<Utc>>> {
        anyhow::bail!("wormhole-slim: cloud object API unavailable")
    }
}
