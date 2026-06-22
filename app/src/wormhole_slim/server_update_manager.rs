//! Minimal [`UpdateManager`] stub for `wormhole-slim` embed builds.

use std::future::Future;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use chrono::{DateTime, Utc};
#[cfg(test)]
pub use crate::wormhole_slim::object_client::GetCloudObjectResponse;
pub use crate::wormhole_slim::object_client::InitialLoadResponse;
use futures::channel::oneshot::{self, Receiver};
use futures::future;
use warpui::r#async::FutureId;
use warpui::{Entity, ModelContext, SingletonEntity};

use crate::ai::agent::conversation::AIConversationId;
use crate::ai::ambient_agents::scheduled::ScheduledAmbientAgent;
use crate::ai::ambient_agents::AmbientAgentTaskId;
use crate::ai::cloud_environments::AmbientAgentEnvironment;
use crate::ai::execution_profiles::AIExecutionProfile;
use crate::ai::facts::AIFact;
#[cfg(not(target_family = "wasm"))]
use crate::ai::mcp::templatable::TemplatableMCPServer;
use crate::cloud_object::model::actions::ObjectActionType;
use crate::cloud_object::model::generic_string_model::{GenericStringModel, GenericStringObjectId, Serializer, StringModel};
use crate::cloud_object::{
    CloudModelType, CloudObjectEventEntrypoint, CloudObjectLocation, GenericCloudObject, Owner,
    Revision,
};
use crate::drive::sharing::SharingAccessLevel;
use crate::drive::CloudObjectTypeAndId;
use crate::env_vars::{CloudEnvVarCollectionModel, EnvVarCollection};
use crate::notebooks::CloudNotebookModel;
use crate::persistence::ModelEvent;
use crate::server::cloud_objects::listener::ObjectUpdateMessage;
use crate::server::ids::{ClientId, HashableId, ObjectUid, ServerId, SyncId, ToServerId};
use crate::settings::cloud_preferences::Preference;
use crate::workflows::workflow::Workflow;
use crate::workflows::workflow_enum::WorkflowEnum;
use crate::cloud_object::Space;
use crate::wormhole_slim::object_client::{SlimObjectClient, StubObjectClient};
use cloud_objects::{AccessLevel, MCPGalleryTemplate};
use cloud_object_client::GuestIdentifier;

#[derive(Debug, PartialEq)]
pub enum OperationSuccessType {
    Success,
    Failure,
    Rejection,
    Denied(String),
    FeatureNotAvailable,
}

#[derive(Debug, PartialEq)]
pub enum ObjectOperation {
    Create { initiated_by: InitiatedBy },
    Update,
    MoveToFolder,
    MoveToDrive,
    Trash,
    TakeEditAccess,
    Untrash,
    Delete { initiated_by: InitiatedBy },
    EmptyTrash,
    UpdatePermissions,
    Leave,
}

#[derive(Debug)]
pub struct ObjectOperationResult {
    pub success_type: OperationSuccessType,
    pub operation: ObjectOperation,
    pub client_id: Option<ClientId>,
    pub server_id: Option<ServerId>,
    pub num_objects: Option<i32>,
}

#[derive(Debug)]
pub enum UpdateManagerEvent {
    ObjectOperationComplete {
        result: ObjectOperationResult,
    },
    CloudPreferencesUpdated {
        updated: Vec<Preference>,
    },
    MCPGalleryUpdated {
        templates: Vec<MCPGalleryTemplate>,
    },
    AmbientTaskUpdated {
        task_id: AmbientAgentTaskId,
        timestamp: DateTime<Utc>,
    },
}

pub enum FetchSingleObjectOption {
    None,
    ForceOverwrite,
    IgnoreIfExists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitiatedBy {
    User,
    System,
}

#[derive(Debug)]
pub struct GenericStringObjectInput<T, S>
where
    T: StringModel<CloudObjectType = GenericCloudObject<GenericStringObjectId, GenericStringModel<T, S>>> + 'static,
    S: Serializer<T> + 'static,
{
    pub id: ClientId,
    pub model: GenericStringModel<T, S>,
    pub initial_folder_id: Option<SyncId>,
    pub entrypoint: CloudObjectEventEntrypoint,
}

pub struct UpdateManager {
    #[allow(dead_code)]
    model_event_sender: Option<SyncSender<ModelEvent>>,
    #[allow(dead_code)]
    object_client: Arc<dyn SlimObjectClient>,
    #[allow(dead_code)]
    spawned_futures: Vec<FutureId>,
}

impl UpdateManager {
    pub fn new(
        model_event_sender: Option<SyncSender<ModelEvent>>,
        _object_client: Arc<dyn crate::server::server_api::object::ObjectClient>,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self {
            model_event_sender,
            object_client: Arc::new(StubObjectClient),
            spawned_futures: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn mock(_ctx: &mut ModelContext<Self>) -> Self {
        Self::new(None, Arc::new(StubObjectClient), _ctx)
    }

    #[cfg(test)]
    pub fn mock_initial_load(
        &mut self,
        _response: InitialLoadResponse,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    #[cfg(any(test, feature = "integration_tests"))]
    pub fn spawned_futures(&self) -> &[FutureId] {
        &self.spawned_futures
    }

    pub fn remove_team_objects(&mut self, _left_team_uid: ServerId, _ctx: &mut ModelContext<Self>) {}

    pub fn resync_object(
        &mut self,
        _cloud_object_type_and_id: &CloudObjectTypeAndId,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn start_polling_for_updated_objects(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn refresh_updated_objects(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn stop_polling_for_updated_objects(&mut self) {}

    pub fn initial_load_complete(&self) -> impl Future<Output = ()> {
        future::ready(())
    }

    pub fn reset_initial_load(&self) {}

    pub fn received_message_from_server(
        &mut self,
        _message: ObjectUpdateMessage,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn fetch_single_cloud_object(
        &mut self,
        _server_id: &ServerId,
        _fetch_single_object_option: FetchSingleObjectOption,
        _ctx: &mut ModelContext<Self>,
    ) -> Receiver<()> {
        let (_tx, rx) = oneshot::channel();
        rx
    }

    pub fn replace_object_with_conflict(&mut self, _uid: &ObjectUid, _ctx: &mut ModelContext<Self>) {}

    pub fn update_ai_fact(
        &mut self,
        _ai_fact: AIFact,
        _ai_fact_id: SyncId,
        _revision_ts: Option<Revision>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn update_templatable_mcp_server(
        &mut self,
        _templatable_mcp_server: TemplatableMCPServer,
        _templatable_mcp_server_id: SyncId,
        _revision_ts: Option<Revision>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn update_workflow(
        &mut self,
        _workflow: Workflow,
        _workflow_id: SyncId,
        _revision_ts: Option<Revision>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn update_workflow_enum(
        &mut self,
        _workflow_enum: WorkflowEnum,
        _workflow_enum_id: SyncId,
        _revision_ts: Option<Revision>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn update_env_var_collection(
        &mut self,
        _env_var_collection: EnvVarCollection,
        _env_var_collection_id: SyncId,
        _revision_ts: Option<Revision>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn update_ambient_agent_environment(
        &mut self,
        _environment: AmbientAgentEnvironment,
        _environment_id: SyncId,
        _revision_ts: Option<Revision>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn update_notebook_data(
        &mut self,
        _data: Arc<String>,
        _notebook_id: SyncId,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn update_notebook_title(
        &mut self,
        _title: Arc<String>,
        _notebook_id: SyncId,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn leave_object(&mut self, _server_id: ServerId, _ctx: &mut ModelContext<Self>) {}

    pub fn set_object_link_permissions(
        &mut self,
        _server_id: ServerId,
        _access_level: Option<SharingAccessLevel>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn add_object_guests(
        &mut self,
        _server_id: ServerId,
        _guest_emails: Vec<String>,
        _access_level: AccessLevel,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn update_object_guests(
        &mut self,
        _server_id: ServerId,
        _guest_emails: Vec<String>,
        _access_level: AccessLevel,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn remove_object_guest(
        &mut self,
        _server_id: ServerId,
        _guest: GuestIdentifier,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn add_ai_conversation_guests(
        &mut self,
        _server_id: ServerId,
        _conversation_id: AIConversationId,
        _guest_emails: Vec<String>,
        _access_level: AccessLevel,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn update_ai_conversation_guests(
        &mut self,
        _server_id: ServerId,
        _conversation_id: AIConversationId,
        _guest_emails: Vec<String>,
        _access_level: AccessLevel,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn remove_ai_conversation_guest(
        &mut self,
        _server_id: ServerId,
        _conversation_id: AIConversationId,
        _guest: GuestIdentifier,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn set_ai_conversation_link_permissions(
        &mut self,
        _server_id: ServerId,
        _conversation_id: AIConversationId,
        _access_level: Option<SharingAccessLevel>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn move_object_to_location(
        &mut self,
        _object_id: CloudObjectTypeAndId,
        _new_location: CloudObjectLocation,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn duplicate_object(
        &mut self,
        _cloud_object_type_and_id: &CloudObjectTypeAndId,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn create_ai_fact(
        &mut self,
        _ai_fact: AIFact,
        _client_id: ClientId,
        _owner: Owner,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn create_templatable_mcp_server(
        &mut self,
        _templatable_mcp_server: TemplatableMCPServer,
        _client_id: ClientId,
        _owner: Owner,
        _initiated_by: InitiatedBy,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn create_ambient_agent_environment(
        &mut self,
        _ambient_agent_environment: AmbientAgentEnvironment,
        _client_id: ClientId,
        _owner: Owner,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn create_ambient_agent_environment_online(
        &mut self,
        _ambient_agent_environment: AmbientAgentEnvironment,
        _client_id: ClientId,
        _owner: Owner,
        _ctx: &mut ModelContext<Self>,
    ) -> impl Future<Output = anyhow::Result<ServerId>> {
        future::ready(Err(anyhow::anyhow!("wormhole-slim stub")))
    }

    pub fn create_scheduled_ambient_agent_online(
        &mut self,
        _scheduled_ambient_agent: ScheduledAmbientAgent,
        _client_id: ClientId,
        _owner: Owner,
        _ctx: &mut ModelContext<Self>,
    ) -> impl Future<Output = anyhow::Result<ServerId>> {
        future::ready(Err(anyhow::anyhow!("wormhole-slim stub")))
    }

    pub fn update_scheduled_ambient_agent_online(
        &mut self,
        _scheduled_ambient_agent: ScheduledAmbientAgent,
        _scheduled_ambient_agent_id: SyncId,
        _revision_ts: Option<Revision>,
        _ctx: &mut ModelContext<Self>,
    ) -> impl Future<Output = anyhow::Result<()>> {
        future::ready(Err(anyhow::anyhow!("wormhole-slim stub")))
    }

    pub fn create_ai_execution_profile(
        &mut self,
        _ai_execution_profile: AIExecutionProfile,
        _client_id: ClientId,
        _owner: Owner,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn update_ai_execution_profile(
        &mut self,
        _ai_execution_profile: AIExecutionProfile,
        _ai_execution_profile_id: SyncId,
        _revision_ts: Option<Revision>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn delete_ai_execution_profile(
        &mut self,
        _ai_execution_profile_id: SyncId,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_notebook(
        &mut self,
        _client_id: ClientId,
        _owner: Owner,
        _initial_folder_id: Option<SyncId>,
        _model: CloudNotebookModel,
        _entrypoint: CloudObjectEventEntrypoint,
        _force_expand: bool,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_workflow(
        &mut self,
        _workflow: Workflow,
        _owner: Owner,
        _initial_folder_id: Option<SyncId>,
        _client_id: ClientId,
        _entrypoint: CloudObjectEventEntrypoint,
        _force_expand: bool,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_workflow_enum(
        &mut self,
        _workflow_enum: WorkflowEnum,
        _owner: Owner,
        _client_id: ClientId,
        _entrypoint: CloudObjectEventEntrypoint,
        _force_expand: bool,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_env_var_collection(
        &mut self,
        _client_id: ClientId,
        _owner: Owner,
        _initial_folder_id: Option<SyncId>,
        _model: CloudEnvVarCollectionModel,
        _entrypoint: CloudObjectEventEntrypoint,
        _force_expand: bool,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_folder(
        &mut self,
        _name: String,
        _owner: Owner,
        _client_id: ClientId,
        _initial_folder_id: Option<SyncId>,
        _force_expand: bool,
        _initiated_by: InitiatedBy,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn bulk_create_generic_string_objects<S, T>(
        &mut self,
        _owner: Owner,
        _inputs: Vec<GenericStringObjectInput<T, S>>,
        _ctx: &mut ModelContext<Self>,
    ) where
        T: StringModel<CloudObjectType = GenericCloudObject<GenericStringObjectId, GenericStringModel<T, S>>> + 'static,
        S: Serializer<T> + 'static,
    {
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_object<K, M>(
        &mut self,
        _model: M,
        _owner: Owner,
        _client_id: ClientId,
        _entrypoint: CloudObjectEventEntrypoint,
        _force_expand: bool,
        _initial_folder_id: Option<SyncId>,
        _initiated_by: InitiatedBy,
        _ctx: &mut ModelContext<Self>,
    ) where
        K: HashableId + ToServerId + std::fmt::Debug + Into<String> + Clone + Copy + Send + Sync + 'static,
        M: CloudModelType<IdType = K, CloudObjectType = GenericCloudObject<K, M>> + 'static,
    {
    }

    pub fn update_object_online<K, M>(
        &mut self,
        _model: M,
        _object_id: SyncId,
        _revision_ts: Option<Revision>,
        _ctx: &mut ModelContext<Self>,
    ) -> impl Future<Output = anyhow::Result<()>>
    where
        K: HashableId + ToServerId + std::fmt::Debug + Into<String> + Clone + Copy + Send + Sync + 'static,
        M: CloudModelType<IdType = K, CloudObjectType = GenericCloudObject<K, M>> + 'static,
    {
        future::ready(Err(anyhow::anyhow!("wormhole-slim stub")))
    }

    pub fn update_object<K, M>(
        &mut self,
        _model: M,
        _object_id: SyncId,
        _revision_ts: Option<Revision>,
        _ctx: &mut ModelContext<Self>,
    ) where
        K: HashableId + ToServerId + std::fmt::Debug + Into<String> + Clone + Copy + Send + Sync + 'static,
        M: CloudModelType<IdType = K, CloudObjectType = GenericCloudObject<K, M>> + 'static,
    {
    }

    pub fn record_object_action(
        &mut self,
        _id_and_type: CloudObjectTypeAndId,
        _action_type: ObjectActionType,
        _data: Option<String>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn grab_notebook_edit_access(
        &mut self,
        _notebook_id: SyncId,
        _optimistically_grant_access: bool,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn give_up_notebook_edit_access(&mut self, _notebook_id: SyncId, _ctx: &mut ModelContext<Self>) {}

    pub fn trash_object(&mut self, _id: CloudObjectTypeAndId, _ctx: &mut ModelContext<Self>) {}

    pub fn untrash_object(&mut self, _id: CloudObjectTypeAndId, _ctx: &mut ModelContext<Self>) {}

    pub fn delete_object_by_user(&mut self, _id: CloudObjectTypeAndId, _ctx: &mut ModelContext<Self>) {}

    pub fn delete_object_with_initiated_by(
        &mut self,
        _id: CloudObjectTypeAndId,
        _initiated_by: InitiatedBy,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn empty_trash(&mut self, _space: Space, _ctx: &mut ModelContext<Self>) {}

    pub fn on_object_delete_success(
        &mut self,
        _deleted_ids: Vec<SyncId>,
        _ctx: &mut ModelContext<'_, UpdateManager>,
    ) -> i32 {
        0
    }

    pub fn rename_folder(
        &mut self,
        _folder_id: SyncId,
        _new_name: String,
        _ctx: &mut ModelContext<Self>,
    ) {
    }
}

pub fn get_duplicate_object_name(original_name: &str) -> String {
    format!("{original_name} (1)")
}

impl Entity for UpdateManager {
    type Event = UpdateManagerEvent;
}

impl SingletonEntity for UpdateManager {}
