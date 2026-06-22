//! AI server API types + stub client for wormhole-slim.

use std::collections::HashMap;

use anyhow::anyhow;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
#[cfg(test)]
use mockall::automock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentTaskState {
    Blocked,
    Cancelled,
    Claimed,
    Error,
    InProgress,
    Succeeded,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlatformErrorCode {
    AuthenticationRequired,
    BudgetExceeded,
    ContentPolicyViolation,
    EnvironmentSetupFailed,
    ExternalAuthenticationRequired,
    FeatureNotAvailable,
    InsufficientCredits,
    IntegrationDisabled,
    IntegrationNotConfigured,
    InternalError,
    InvalidRequest,
    NotAuthorized,
    ResourceUnavailable,
    ResourceNotFound,
}

pub use crate::ai::agent::api::ServerConversationToken;
pub use crate::ai::agent::conversation::{
    AIAgentConversationFormat, ServerAIConversationMetadata,
};
pub use crate::ai::ambient_agents::AmbientAgentTaskId;
// Re-export ambient agent types for backwards compatibility
pub use crate::ai::ambient_agents::{
    task::{AttachmentInput, TaskAttachment},
    AgentConfigSnapshot, AgentSource, AmbientAgentTask, AmbientAgentTaskState, TaskStatusMessage,
};
use super::harness_support::{UploadField, UploadTarget};
use crate::ai::artifacts::Artifact;
use warp_core::report_error;
use crate::ai::generate_code_review_content::api::{
    GenerateCodeReviewContentRequest, GenerateCodeReviewContentResponse,
};
pub use crate::ai::harness_availability::HarnessAvailability;
pub use ai::index::full_source_code_embedding::{
    self, ContentHash, EmbeddingConfig, NodeHash, RepoMetadata,
};
pub use ai::index::full_source_code_embedding::store_client::IntermediateNode;
use crate::ai::llms::{
    AvailableLLMs, DisableReason, LLMContextWindow, LLMInfo, LLMModelHost, LLMProvider, LLMSpec,
    LLMUsageMetadata, RoutingHostConfig,
};
pub use crate::ai::llms::ModelsByFeature;
#[cfg(feature = "agent_mode_evals")]
use crate::ai::request_usage_model::RequestLimitInfo;
#[cfg(not(feature = "agent_mode_evals"))]
use crate::ai::BonusGrant;
use crate::ai::RequestUsageInfo;
pub use crate::ai_assistant::execution_context::WarpAiExecutionContext;
pub use crate::ai_assistant::requests::GenerateDialogueResult;
pub use crate::ai_assistant::utils::TranscriptPart;
pub use crate::ai_assistant::{AIGeneratedCommand, GenerateCommandsFromNaturalLanguageError};
pub use crate::drive::workflows::ai_assist::{
    GeneratedCommandMetadata, GeneratedCommandMetadataError,
};
use crate::persistence::model::ConversationUsageMetadata;

pub use crate::ai::agent::UserQueryMode;
pub use crate::terminal::model::block::SerializedBlock;
pub use warp_multi_agent_api::ConversationData;

#[cfg(not(feature = "agent_mode_evals"))]
use crate::{
    ai::request_usage_model::BonusGrantScope,
    server::ids::ServerId,
    workspaces::{gql_convert::PLACEHOLDER_WORKSPACE_UID, workspace::WorkspaceUid},
};

#[derive(Debug, Clone)]
pub struct ConversationUsage {
    pub conversation_id: String,
    pub last_updated: DateTime<Utc>,
    pub title: String,
    pub usage_metadata: ConversationUsageMetadata,
}

#[derive(Debug, Clone, Default)]
pub struct ScheduledAgentHistory {
    pub entries: Vec<serde_json::Value>,
    pub last_ran: Option<warp_graphql::scalars::Time>,
    pub next_run: Option<warp_graphql::scalars::Time>,
}

const AI_ASSISTANT_REQUEST_TIMEOUT_SECONDS: u64 = 30;

/// A status update for a task, optionally including a platform error code.
pub struct TaskStatusUpdate {
    pub message: String,
    pub error_code: Option<PlatformErrorCode>,
}
fn public_api_user_query_mode(mode: UserQueryMode) -> &'static str {
    match mode {
        UserQueryMode::Normal => "normal",
        UserQueryMode::Plan => "plan",
        UserQueryMode::Orchestrate => "orchestrate",
    }
}

fn serialize_user_query_mode_for_public_api<S>(
    mode: &UserQueryMode,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(public_api_user_query_mode(*mode))
}

impl TaskStatusUpdate {
    /// Create a status update with just a message (no error code).
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_code: None,
        }
    }

    /// Create a status update with a message and error code.
    pub fn with_error_code(message: impl Into<String>, error_code: PlatformErrorCode) -> Self {
        Self {
            message: message.into(),
            error_code: Some(error_code),
        }
    }
}

/// JSON payload sent to the public `POST /agent/run` API.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SpawnAgentRequest {
    /// None for skill-only or conversation-only invocations; omitted on the wire.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// The public API accepts lowercase mode strings (`normal`, `plan`, or `orchestrate`).
    #[serde(serialize_with = "serialize_user_query_mode_for_public_api")]
    pub mode: UserQueryMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<AgentConfigSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub team: Option<bool>,
    /// Agent identity UID to use as the execution principal for the run.
    #[serde(rename = "agent_identity_uid", skip_serializing_if = "Option::is_none")]
    pub agent_identity_uid: Option<String>,
    /// Use a Claude-compatible skill as the base prompt.
    /// Format: "repo:skill_name" or just "skill_name".
    /// The skill is resolved at runtime in the agent environment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<AttachmentInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interactive: Option<bool>,
    /// Populated when a cloud agent spawns a child run via the public API.
    /// Not yet wired through the local start_agent flow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<String>,
    /// Base64-encoded `warp.multi_agent.v1.Skill` payloads to restore as runtime skills.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub runtime_skills: Vec<String>,
    /// Base64-encoded `warp.multi_agent.v1.Attachment` payloads to restore as referenced attachments.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub referenced_attachments: Vec<String>,
    /// Server-side conversation id to resume against (sets `task.AgentConversationID`).
    /// For local-to-cloud handoff this is the forked conversation id returned by
    /// `POST /agent/conversations/{conversation_id}/fork` at chip-click time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    /// References a batch of files previously uploaded to handoff/{token}/
    /// via `POST /agent/handoff/upload-snapshot`. The server stores the token on the new run's
    /// queued execution input and resolves the prefix in place at rehydration time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_snapshot_token: Option<InitialSnapshotToken>,
    /// When `Some(true)`, the cloud agent skips the end-of-run snapshot upload.
    /// Set by the client when cloud conversation storage is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_disabled: Option<bool>,
    /// True when the source conversation was part of an orchestration tree at
    /// handoff time. Only set on local-to-cloud handoff spawns from an
    /// orchestrated source; absent otherwise. The server uses it to inject the
    /// universal hidden first-turn orchestration handoff message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orchestration_handoff: Option<bool>,
}

/// Server-minted token returned by `POST /agent/handoff/upload-snapshot` that scopes a batch
/// of presigned upload URLs to `handoff/{token}/`. The client passes it
/// back via `SpawnAgentRequest.initial_snapshot_token`; the server stores it on the new run's
/// queued execution input so rehydration discovery can read the same prefix.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InitialSnapshotToken(String);

impl InitialSnapshotToken {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Request body for `POST /agent/handoff/upload-snapshot`. Used by the local-to-cloud
/// handoff flow to allocate a token and presigned upload URLs scoped to
/// `handoff/{token}/` before any task exists.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UploadLocalHandoffSnapshotRequest {
    pub files: Vec<SnapshotUploadFileInfo>,
}

/// Describes a single file the client wants to upload as part of a handoff snapshot.
/// Wire-compatible with the server's `SnapshotUploadFileInfo` schema (also used by the
/// existing harness-side `/harness-support/upload-snapshot` endpoint).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SnapshotUploadFileInfo {
    pub filename: String,
    pub mime_type: String,
}

/// Response body for `POST /agent/handoff/upload-snapshot`. The `uploads` array is aligned
/// by index with the request `files` array; the client matches each `UploadTarget` back
/// to the requested filename by index.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct UploadLocalHandoffSnapshotResponse {
    pub initial_snapshot_token: InitialSnapshotToken,
    pub expires_at: String,
    pub uploads: Vec<UploadTarget>,
}

/// Request body for `POST /agent/conversations/{conversation_id}/fork`.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ForkConversationRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

/// Response body for `POST /agent/conversations/{conversation_id}/fork`. The returned id is sent
/// on the subsequent `POST /agent/runs` request under `conversation_id` (resume semantics).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ForkConversationResponse {
    pub forked_conversation_id: String,
}

/// Request body for `POST /agent/conversations/{conversation_id}/rename`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RenameConversationRequest {
    pub title: String,
}

/// Response body for `POST /agent/conversations/{conversation_id}/rename`.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct RenameConversationResponse {
    pub title: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RunFollowupRequest {
    pub message: String,
}

// --- Orchestrations V2 messaging types ---

#[derive(Debug, Clone, serde::Serialize)]
pub struct SendAgentMessageRequest {
    pub to: Vec<String>,
    pub subject: String,
    pub body: String,
    pub sender_run_id: String,
}

#[derive(Debug, Clone)]
pub struct ListAgentMessagesRequest {
    pub unread_only: bool,
    pub since: Option<String>,
    pub limit: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SendAgentMessageResponse {
    pub message_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentMessageHeader {
    pub message_id: String,
    pub sender_run_id: String,
    pub subject: String,
    pub sent_at: String,
    pub delivered_at: Option<String>,
    pub read_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AgentRunEvent {
    pub event_type: String,
    pub run_id: String,
    pub ref_id: Option<String>,
    pub execution_id: Option<String>,
    pub occurred_at: String,
    pub sequence: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ReportAgentEventRequest {
    pub event_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReportAgentEventResponse {
    pub sequence: i64,
}
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentRunClientEventRequest {
    pub event_uuid: String,
    pub event_name: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<AgentRunClientEventPayload>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum AgentRunClientEventPayload {
    SetupMetric(AgentRunClientSetupMetricPayload),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentRunClientSetupMetricPayload {
    pub start_ts: DateTime<Utc>,
    pub finish_ts: DateTime<Utc>,
    pub latency_ms: i64,
    pub is_error: bool,
}

impl AgentRunClientEventRequest {
    pub fn timeline_event(event_name: impl Into<String>, timestamp: DateTime<Utc>) -> Self {
        Self {
            event_uuid: uuid::Uuid::new_v4().to_string(),
            event_name: event_name.into(),
            timestamp,
            payload: None,
        }
    }

    pub fn setup_metric_event(
        event_name: impl Into<String>,
        start_timestamp: DateTime<Utc>,
        finish_timestamp: DateTime<Utc>,
        is_error: bool,
    ) -> Self {
        Self {
            event_uuid: uuid::Uuid::new_v4().to_string(),
            event_name: event_name.into(),
            timestamp: finish_timestamp,
            payload: Some(AgentRunClientEventPayload::SetupMetric(
                AgentRunClientSetupMetricPayload {
                    start_ts: start_timestamp,
                    finish_ts: finish_timestamp,
                    latency_ms: finish_timestamp
                        .signed_duration_since(start_timestamp)
                        .num_milliseconds()
                        .max(0),
                    is_error,
                },
            )),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReadAgentMessageResponse {
    pub message_id: String,
    pub sender_run_id: String,
    pub subject: String,
    pub body: String,
    pub sent_at: String,
    pub delivered_at: Option<String>,
    pub read_at: Option<String>,
}

#[derive(serde::Deserialize)]
pub struct SpawnAgentResponse {
    pub task_id: AmbientAgentTaskId,
    pub run_id: String,
    #[serde(default)]
    pub at_capacity: bool,
}

/// Response from the artifact endpoint.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(tag = "artifact_type")]
pub enum ArtifactDownloadResponse {
    #[serde(rename = "SCREENSHOT")]
    Screenshot {
        #[serde(flatten)]
        common: ArtifactDownloadCommonFields,
        data: ScreenshotArtifactResponseData,
    },
    #[serde(rename = "FILE")]
    File {
        #[serde(flatten)]
        common: ArtifactDownloadCommonFields,
        data: FileArtifactResponseData,
    },
}

impl ArtifactDownloadResponse {
    fn common(&self) -> &ArtifactDownloadCommonFields {
        match self {
            ArtifactDownloadResponse::Screenshot { common, .. }
            | ArtifactDownloadResponse::File { common, .. } => common,
        }
    }

    pub fn artifact_uid(&self) -> &str {
        &self.common().artifact_uid
    }

    pub fn artifact_type(&self) -> &'static str {
        match self {
            ArtifactDownloadResponse::Screenshot { .. } => "SCREENSHOT",
            ArtifactDownloadResponse::File { .. } => "FILE",
        }
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.common().created_at
    }

    pub fn download_url(&self) -> &str {
        match self {
            ArtifactDownloadResponse::Screenshot { data, .. } => &data.download_url,
            ArtifactDownloadResponse::File { data, .. } => &data.download_url,
        }
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        match self {
            ArtifactDownloadResponse::Screenshot { data, .. } => data.expires_at,
            ArtifactDownloadResponse::File { data, .. } => data.expires_at,
        }
    }

    pub fn content_type(&self) -> &str {
        match self {
            ArtifactDownloadResponse::Screenshot { data, .. } => &data.content_type,
            ArtifactDownloadResponse::File { data, .. } => &data.content_type,
        }
    }

    pub fn filepath(&self) -> Option<&str> {
        match self {
            ArtifactDownloadResponse::Screenshot { .. } => None,
            ArtifactDownloadResponse::File { data, .. } => Some(&data.filepath),
        }
    }

    pub fn filename(&self) -> Option<&str> {
        match self {
            ArtifactDownloadResponse::Screenshot { .. } => None,
            ArtifactDownloadResponse::File { data, .. } => Some(&data.filename),
        }
    }

    pub fn description(&self) -> Option<&str> {
        match self {
            ArtifactDownloadResponse::Screenshot { data, .. } => data.description.as_deref(),
            ArtifactDownloadResponse::File { data, .. } => data.description.as_deref(),
        }
    }

    pub fn size_bytes(&self) -> Option<i64> {
        match self {
            ArtifactDownloadResponse::Screenshot { .. } => None,
            ArtifactDownloadResponse::File { data, .. } => data.size_bytes,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ArtifactDownloadCommonFields {
    pub artifact_uid: String,
    pub created_at: DateTime<Utc>,
}

/// Screenshot-specific data from the artifact endpoint.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ScreenshotArtifactResponseData {
    pub download_url: String,
    pub expires_at: DateTime<Utc>,
    pub content_type: String,
    pub description: Option<String>,
}

/// File-specific data from the artifact endpoint.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FileArtifactResponseData {
    pub download_url: String,
    pub expires_at: DateTime<Utc>,
    pub content_type: String,
    pub filepath: String,
    pub filename: String,
    pub description: Option<String>,
    pub size_bytes: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AttachmentFileInfo {
    pub filename: String,
    pub mime_type: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PrepareAttachmentUploadsRequest {
    pub files: Vec<AttachmentFileInfo>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DownloadAttachmentsRequest {
    pub attachment_ids: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AttachmentDownloadInfo {
    pub attachment_id: String,
    pub download_url: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DownloadAttachmentsResponse {
    pub attachments: Vec<AttachmentDownloadInfo>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct HandoffSnapshotAttachmentInfo {
    pub attachment_id: String,
    pub filename: String,
    pub download_url: String,
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct ListHandoffSnapshotAttachmentsResponse {
    pub attachments: Vec<HandoffSnapshotAttachmentInfo>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct AttachmentUploadInfo {
    pub attachment_id: String,
    pub upload_url: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PrepareAttachmentUploadsResponse {
    pub attachments: Vec<AttachmentUploadInfo>,
}

#[derive(Debug, Clone)]
pub struct CreateFileArtifactUploadRequest {
    pub conversation_id: Option<String>,
    pub run_id: Option<String>,
    pub filepath: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct FileArtifactRecord {
    pub artifact_uid: String,
    pub filepath: String,
    pub description: Option<String>,
    pub mime_type: String,
    pub size_bytes: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct FileArtifactUploadHeaderInfo {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct FileArtifactUploadTargetInfo {
    pub url: String,
    pub method: String,
    pub headers: Vec<FileArtifactUploadHeaderInfo>,
    /// Ordered multipart form fields for presigned POST uploads.
    pub fields: Vec<UploadField>,
}

#[derive(Debug, Clone)]
pub struct CreateFileArtifactUploadResponse {
    pub artifact: FileArtifactRecord,
    pub upload_target: FileArtifactUploadTargetInfo,
}

/// A single git credential entry returned by `taskGitCredentials`.
#[derive(Clone)]
pub struct GitCredential {
    /// The GitHub token (OAuth user token or App installation token).
    pub token: String,
    /// The GitHub username. `None` for service-account (installation token) principals.
    pub username: Option<String>,
    /// The GitHub email. `None` for service-account principals.
    pub email: Option<String>,
    /// The host (always `"github.com"` in V1).
    pub host: String,
}

/// Filter parameters for listing ambient agent tasks.
#[derive(Clone, Debug, Default)]
pub struct TaskListFilter {
    pub creator_uid: Option<String>,
    pub updated_after: Option<DateTime<Utc>>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub states: Option<Vec<AmbientAgentTaskState>>,
    pub source: Option<AgentSource>,
    pub execution_location: Option<ExecutionLocation>,
    pub environment_id: Option<String>,
    pub skill_spec: Option<String>,
    pub schedule_id: Option<String>,
    pub ancestor_run_id: Option<String>,
    pub config_name: Option<String>,
    pub model_id: Option<String>,
    pub artifact_type: Option<ArtifactType>,
    pub search_query: Option<String>,
    pub sort_by: Option<RunSortBy>,
    pub sort_order: Option<RunSortOrder>,
    pub cursor: Option<String>,
}

/// Execution location filter values accepted by the public API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionLocation {
    Local,
    Remote,
}

impl ExecutionLocation {
    pub fn as_query_param(&self) -> &'static str {
        match self {
            ExecutionLocation::Local => "LOCAL",
            ExecutionLocation::Remote => "REMOTE",
        }
    }
}

/// Artifact type filter values accepted by the public API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtifactType {
    Plan,
    PullRequest,
    Screenshot,
    File,
}

impl ArtifactType {
    pub fn as_query_param(&self) -> &'static str {
        match self {
            ArtifactType::Plan => "PLAN",
            ArtifactType::PullRequest => "PULL_REQUEST",
            ArtifactType::Screenshot => "SCREENSHOT",
            ArtifactType::File => "FILE",
        }
    }
}

/// Sort-by values accepted by the public API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunSortBy {
    UpdatedAt,
    CreatedAt,
    Title,
    Agent,
}

impl RunSortBy {
    pub fn as_query_param(&self) -> &'static str {
        match self {
            RunSortBy::UpdatedAt => "updated_at",
            RunSortBy::CreatedAt => "created_at",
            RunSortBy::Title => "title",
            RunSortBy::Agent => "agent",
        }
    }
}

/// Sort-order values accepted by the public API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunSortOrder {
    Asc,
    Desc,
}

impl RunSortOrder {
    pub fn as_query_param(&self) -> &'static str {
        match self {
            RunSortOrder::Asc => "asc",
            RunSortOrder::Desc => "desc",
        }
    }
}

/// Build the path + query string for `GET /api/v1/agent/runs` from a filter.
pub(crate) fn build_list_agent_runs_url(limit: i32, filter: &TaskListFilter) -> String {
    let mut url = format!("agent/runs?limit={limit}");

    let mut push = |key: &str, value: &str| {
        url.push('&');
        url.push_str(key);
        url.push('=');
        url.push_str(urlencoding::encode(value).as_ref());
    };

    if let Some(creator_uid) = filter.creator_uid.as_deref() {
        push("creator", creator_uid);
    }
    if let Some(updated_after) = filter.updated_after {
        push("updated_after", &updated_after.to_rfc3339());
    }
    if let Some(created_after) = filter.created_after {
        push("created_after", &created_after.to_rfc3339());
    }
    if let Some(created_before) = filter.created_before {
        push("created_before", &created_before.to_rfc3339());
    }
    if let Some(states) = filter.states.as_ref() {
        for state in states {
            if let Some(value) = state.as_query_param() {
                push("state", value);
            }
        }
    }
    if let Some(source) = filter.source.as_ref() {
        push("source", source.as_str());
    }
    if let Some(execution_location) = filter.execution_location {
        push("execution_location", execution_location.as_query_param());
    }
    if let Some(environment_id) = filter.environment_id.as_deref() {
        push("environment_id", environment_id);
    }
    if let Some(skill_spec) = filter.skill_spec.as_deref() {
        push("skill_spec", skill_spec);
    }
    if let Some(schedule_id) = filter.schedule_id.as_deref() {
        push("schedule_id", schedule_id);
    }
    if let Some(ancestor_run_id) = filter.ancestor_run_id.as_deref() {
        push("ancestor_run_id", ancestor_run_id);
    }
    if let Some(config_name) = filter.config_name.as_deref() {
        push("name", config_name);
    }
    if let Some(model_id) = filter.model_id.as_deref() {
        push("model_id", model_id);
    }
    if let Some(artifact_type) = filter.artifact_type {
        push("artifact_type", artifact_type.as_query_param());
    }
    if let Some(search_query) = filter.search_query.as_deref() {
        push("q", search_query);
    }
    if let Some(sort_by) = filter.sort_by {
        push("sort_by", sort_by.as_query_param());
    }
    if let Some(sort_order) = filter.sort_order {
        push("sort_order", sort_order.as_query_param());
    }
    if let Some(cursor) = filter.cursor.as_deref() {
        push("cursor", cursor);
    }

    url
}

pub(crate) fn build_run_followup_url(run_id: &AmbientAgentTaskId) -> String {
    format!("agent/runs/{run_id}/followups")
}
pub(crate) fn build_fork_conversation_url(conversation_id: &str) -> String {
    format!(
        "agent/conversations/{}/fork",
        urlencoding::encode(conversation_id)
    )
}

pub(crate) fn build_rename_conversation_url(conversation_id: &str) -> String {
    format!(
        "agent/conversations/{}/rename",
        urlencoding::encode(conversation_id)
    )
}

struct ListRunsResponse {
    runs: Vec<AmbientAgentTask>,
}

impl<'de> serde::Deserialize<'de> for ListRunsResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct RawResponse {
            runs: Vec<serde_json::Value>,
        }

        let raw = RawResponse::deserialize(deserializer)?;
        let mut runs = Vec::with_capacity(raw.runs.len());

        for task_value in raw.runs.into_iter() {
            match serde_json::from_value::<AmbientAgentTask>(task_value) {
                Ok(task) => runs.push(task),
                Err(e) => {
                    // Log the error and skip this task instead of failing the entire request
                    report_error!(anyhow!("Failed to deserialize ambient agent task: {}", e));
                }
            }
        }

        Ok(ListRunsResponse { runs })
    }
}

/// Source information for an agent skill.
#[derive(Clone, serde::Deserialize, Debug, PartialEq)]
pub struct AgentSkillSource {
    pub owner: String,
    pub name: String,
    pub skill_path: String,
}

/// Environment information for an agent skill.
#[derive(Clone, serde::Deserialize, Debug, PartialEq)]
pub struct AgentSkillEnvironment {
    pub uid: String,
    pub name: String,
}

/// A variant of an agent skill.
#[derive(Clone, serde::Deserialize, Debug, PartialEq)]
pub struct AgentSkillVariant {
    pub id: String,
    pub description: String,
    pub base_prompt: String,
    pub source: AgentSkillSource,
    pub environments: Vec<AgentSkillEnvironment>,
}

/// An agent skill item with its variants.
#[derive(Clone, serde::Deserialize, Debug, PartialEq)]
pub struct AgentSkillItem {
    pub name: String,
    pub variants: Vec<AgentSkillVariant>,
}

#[derive(serde::Deserialize)]
struct ListSkillsResponse {
    agents: Vec<AgentSkillItem>,
}

/// Reference to a managed secret by name.
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct SecretRef {
    pub name: String,
}

/// JSON payload sent to `POST /agent/identities`.
#[derive(Clone, serde::Serialize, Debug, PartialEq, Eq)]
pub struct CreateAgentRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<SecretRef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_id: Option<String>,
}

/// JSON payload sent to `PUT /agent/identities/{uid}`.
#[derive(Clone, Default, serde::Serialize, Debug, PartialEq, Eq)]
pub struct UpdateAgentRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Vec<SecretRef>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_id: Option<String>,
}

/// Public API representation of a named agent identity.
#[derive(Clone, serde::Deserialize, serde::Serialize, Debug, PartialEq, Eq)]
pub struct AgentResponse {
    pub uid: String,
    pub name: String,
    pub description: Option<String>,
    pub available: bool,
    pub created_at: DateTime<Utc>,
    pub secrets: Vec<SecretRef>,
    pub skills: Vec<String>,
    pub base_model: Option<String>,
    #[serde(default)]
    pub environment_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct ListAgentsResponse {
    agents: Vec<AgentResponse>,
}
fn build_agent_url(uid: &str) -> String {
    format!("agent/identities/{}", urlencoding::encode(uid))
}

#[derive(Clone, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct ConnectedSelfHostedWorker {
    pub worker_host: String,
    pub connection_count: u32,
    pub connected_at: String,
    pub last_seen_at: String,
}

#[derive(Clone, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct ListConnectedSelfHostedWorkersResponse {
    pub workers: Vec<ConnectedSelfHostedWorker>,
}

pub(crate) const CONNECTED_SELF_HOSTED_WORKERS_PATH: &str = "agent/connected-self-hosted-workers";

#[cfg_attr(test, automock)]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait AIClient: 'static + Send + Sync {
    async fn generate_commands_from_natural_language(
        &self,
        prompt: String,
        ai_execution_context: Option<WarpAiExecutionContext>,
    ) -> Result<Vec<AIGeneratedCommand>, GenerateCommandsFromNaturalLanguageError>;

    async fn generate_dialogue_answer(
        &self,
        transcript: Vec<TranscriptPart>,
        prompt: String,
        ai_execution_context: Option<WarpAiExecutionContext>,
    ) -> anyhow::Result<GenerateDialogueResult>;

    async fn generate_metadata_for_command(
        &self,
        command: String,
    ) -> Result<GeneratedCommandMetadata, GeneratedCommandMetadataError>;

    async fn get_request_limit_info(&self) -> Result<RequestUsageInfo, anyhow::Error>;

    /// Returns conversation usage history for the current user over the requested number of days.
    ///
    /// If `last_updated_end_timestamp` is provided, only conversations updated before that timestamp are returned.
    async fn get_conversation_usage_history(
        &self,
        days: Option<i32>,
        limit: Option<i32>,
        last_updated_end_timestamp: Option<DateTime<Utc>>,
    ) -> Result<Vec<ConversationUsage>, anyhow::Error>;

    async fn get_feature_model_choices(&self) -> Result<ModelsByFeature, anyhow::Error>;

    async fn get_available_harnesses(&self) -> Result<Vec<HarnessAvailability>, anyhow::Error>;
    async fn list_connected_self_hosted_workers(
        &self,
    ) -> Result<ListConnectedSelfHostedWorkersResponse, anyhow::Error>;

    /// Fetches the free-tier available models without requiring authentication.
    /// Used during pre-login onboarding so logged-out users see an accurate model list
    /// instead of the hard-coded `ModelsByFeature::default()` fallback.
    async fn get_free_available_models(
        &self,
        referrer: Option<String>,
    ) -> Result<ModelsByFeature, anyhow::Error>;

    async fn update_merkle_tree(
        &self,
        embedding_config: EmbeddingConfig,
        nodes: Vec<IntermediateNode>,
    ) -> anyhow::Result<HashMap<NodeHash, bool>>;

    async fn generate_code_embeddings(
        &self,
        embedding_config: EmbeddingConfig,
        fragments: Vec<full_source_code_embedding::Fragment>,
        root_hash: NodeHash,
        repo_metadata: RepoMetadata,
    ) -> anyhow::Result<HashMap<ContentHash, bool>>;

    async fn provide_negative_feedback_response_for_ai_conversation(
        &self,
        conversation_id: String,
        request_ids: Vec<String>,
    ) -> anyhow::Result<i32, anyhow::Error>;

    async fn create_agent_task(
        &self,
        prompt: String,
        environment_uid: Option<String>,
        parent_run_id: Option<String>,
        config: Option<AgentConfigSnapshot>,
    ) -> anyhow::Result<AmbientAgentTaskId, anyhow::Error>;

    async fn update_agent_task(
        &self,
        task_id: AmbientAgentTaskId,
        task_state: Option<AgentTaskState>,
        session_id: Option<session_sharing_protocol::common::SessionId>,
        conversation_id: Option<String>,
        status_message: Option<TaskStatusUpdate>,
    ) -> anyhow::Result<(), anyhow::Error>;

    async fn spawn_agent(
        &self,
        request: SpawnAgentRequest,
    ) -> anyhow::Result<SpawnAgentResponse, anyhow::Error>;

    /// Allocate an initial snapshot token and presigned upload URLs for staging local-to-cloud
    /// handoff snapshot files before the corresponding cloud task exists.
    async fn upload_local_handoff_snapshot(
        &self,
        request: UploadLocalHandoffSnapshotRequest,
    ) -> anyhow::Result<UploadLocalHandoffSnapshotResponse, anyhow::Error>;

    /// Materialize a server-side fork of a conversation.
    async fn fork_conversation(
        &self,
        conversation_id: String,
        title: Option<String>,
    ) -> anyhow::Result<ForkConversationResponse, anyhow::Error>;

    /// Rename a server-side conversation and return the normalized title.
    async fn rename_conversation(
        &self,
        conversation_id: String,
        title: String,
    ) -> anyhow::Result<RenameConversationResponse, anyhow::Error>;

    async fn list_ambient_agent_tasks(
        &self,
        limit: i32,
        filter: TaskListFilter,
    ) -> anyhow::Result<Vec<AmbientAgentTask>, anyhow::Error>;

    /// List agent runs and return the raw server JSON response.
    async fn list_agent_runs_raw(
        &self,
        limit: i32,
        filter: TaskListFilter,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error>;

    async fn get_ambient_agent_task(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<AmbientAgentTask, anyhow::Error>;

    /// Fetch a single agent run and return the raw server JSON response.
    async fn get_agent_run_raw(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error>;

    async fn submit_run_followup(
        &self,
        run_id: &AmbientAgentTaskId,
        request: RunFollowupRequest,
    ) -> anyhow::Result<(), anyhow::Error>;

    async fn get_scheduled_agent_history(
        &self,
        schedule_id: &str,
    ) -> anyhow::Result<ScheduledAgentHistory, anyhow::Error>;

    async fn get_ai_conversation(
        &self,
        server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<(ConversationData, ServerAIConversationMetadata), anyhow::Error>;

    async fn list_ai_conversation_metadata(
        &self,
        conversation_ids: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<ServerAIConversationMetadata>>;

    async fn get_ai_conversation_format(
        &self,
        server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<AIAgentConversationFormat, anyhow::Error>;

    async fn get_block_snapshot(
        &self,
        server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<SerializedBlock, anyhow::Error>;

    async fn delete_ai_conversation(
        &self,
        server_conversation_token: String,
    ) -> anyhow::Result<(), anyhow::Error>;

    async fn list_skills(
        &self,
        repo: Option<String>,
    ) -> anyhow::Result<Vec<AgentSkillItem>, anyhow::Error>;

    async fn list_agents(&self) -> anyhow::Result<Vec<AgentResponse>, anyhow::Error>;

    async fn list_agents_raw(&self) -> anyhow::Result<serde_json::Value, anyhow::Error>;

    async fn get_agent(&self, uid: &str) -> anyhow::Result<AgentResponse, anyhow::Error>;

    async fn get_agent_raw(&self, uid: &str) -> anyhow::Result<serde_json::Value, anyhow::Error>;

    async fn create_agent(
        &self,
        request: CreateAgentRequest,
    ) -> anyhow::Result<AgentResponse, anyhow::Error>;

    async fn create_agent_raw(
        &self,
        request: CreateAgentRequest,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error>;

    async fn update_agent(
        &self,
        uid: &str,
        request: UpdateAgentRequest,
    ) -> anyhow::Result<AgentResponse, anyhow::Error>;

    async fn update_agent_raw(
        &self,
        uid: &str,
        request: UpdateAgentRequest,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error>;

    async fn delete_agent(&self, uid: &str) -> anyhow::Result<(), anyhow::Error>;

    async fn cancel_ambient_agent_task(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<(), anyhow::Error>;

    async fn get_task_git_credentials(
        &self,
        task_id: String,
        workload_token: String,
    ) -> anyhow::Result<Vec<GitCredential>, anyhow::Error>;

    async fn get_task_attachments(
        &self,
        task_id: String,
    ) -> anyhow::Result<Vec<TaskAttachment>, anyhow::Error>;

    async fn create_file_artifact_upload_target(
        &self,
        request: CreateFileArtifactUploadRequest,
    ) -> anyhow::Result<CreateFileArtifactUploadResponse, anyhow::Error>;

    async fn confirm_file_artifact_upload(
        &self,
        artifact_uid: String,
        checksum: String,
    ) -> anyhow::Result<FileArtifactRecord, anyhow::Error>;

    async fn get_artifact_download(
        &self,
        artifact_uid: &str,
    ) -> anyhow::Result<ArtifactDownloadResponse, anyhow::Error>;

    async fn prepare_attachments_for_upload(
        &self,
        task_id: &AmbientAgentTaskId,
        files: &[AttachmentFileInfo],
    ) -> anyhow::Result<PrepareAttachmentUploadsResponse, anyhow::Error>;

    async fn download_task_attachments(
        &self,
        task_id: &AmbientAgentTaskId,
        attachment_ids: &[String],
    ) -> anyhow::Result<DownloadAttachmentsResponse, anyhow::Error>;

    async fn get_handoff_snapshot_attachments(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<Vec<TaskAttachment>, anyhow::Error>;

    // --- Orchestrations V2 messaging ---

    async fn send_agent_message(
        &self,
        request: SendAgentMessageRequest,
    ) -> anyhow::Result<SendAgentMessageResponse, anyhow::Error>;

    async fn list_agent_messages(
        &self,
        run_id: &str,
        request: ListAgentMessagesRequest,
    ) -> anyhow::Result<Vec<AgentMessageHeader>, anyhow::Error>;

    /// Persists the latest observed event sequence number for a run on the
    /// server. Used to keep the server-side cursor in sync with the client so
    /// that driver/cloud restores can resume without replaying events the
    /// parent has already acted on.
    async fn update_event_sequence_on_server(
        &self,
        run_id: &str,
        sequence: i64,
    ) -> anyhow::Result<(), anyhow::Error>;

    async fn report_agent_event(
        &self,
        run_id: &str,
        request: ReportAgentEventRequest,
    ) -> anyhow::Result<ReportAgentEventResponse, anyhow::Error>;
    async fn post_agent_run_client_event(
        &self,
        run_id: &AmbientAgentTaskId,
        request: AgentRunClientEventRequest,
    ) -> anyhow::Result<(), anyhow::Error>;

    async fn mark_message_delivered(&self, message_id: &str) -> anyhow::Result<(), anyhow::Error>;

    async fn read_agent_message(
        &self,
        message_id: &str,
    ) -> anyhow::Result<ReadAgentMessageResponse, anyhow::Error>;

    /// Fetch a normalized conversation by conversation ID.
    async fn get_public_conversation(
        &self,
        conversation_id: &str,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error>;

    /// Fetch a normalized conversation by run ID.
    async fn get_run_conversation(
        &self,
        run_id: &str,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error>;

    /// Generates AI copy for code-review flows: commit messages at dialog-open
    /// time and PR titles / bodies at confirm time. `output_type` in the
    /// request picks which of the three the server returns.
    async fn generate_code_review_content(
        &self,
        request: GenerateCodeReviewContentRequest,
    ) -> Result<GenerateCodeReviewContentResponse, anyhow::Error>;
}


#[derive(Debug, Default)]
pub struct StubAIClient;

#[async_trait]
impl AIClient for StubAIClient {
    async fn generate_commands_from_natural_language(
        &self,
        prompt: String,
        ai_execution_context: Option<WarpAiExecutionContext>,
    ) -> Result<Vec<AIGeneratedCommand>, GenerateCommandsFromNaturalLanguageError> {
        let _ = (prompt, ai_execution_context);
        Err(GenerateCommandsFromNaturalLanguageError::Other)
    }

    async fn generate_dialogue_answer(
        &self,
        transcript: Vec<TranscriptPart>,
        prompt: String,
        ai_execution_context: Option<WarpAiExecutionContext>,
    ) -> anyhow::Result<GenerateDialogueResult> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn generate_metadata_for_command(
        &self,
        command: String,
    ) -> Result<GeneratedCommandMetadata, GeneratedCommandMetadataError> {
        let _ = command;
        Err(GeneratedCommandMetadataError::Other)
    }

    async fn get_conversation_usage_history(
        &self,
        days: Option<i32>,
        limit: Option<i32>,
        last_updated_end_timestamp: Option<DateTime<Utc>>,
    ) -> Result<Vec<ConversationUsage>, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_feature_model_choices(&self) -> Result<ModelsByFeature, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_available_harnesses(&self) -> Result<Vec<HarnessAvailability>, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_free_available_models(
        &self,
        referrer: Option<String>,
    ) -> Result<ModelsByFeature, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn update_merkle_tree(
        &self,
        embedding_config: EmbeddingConfig,
        nodes: Vec<IntermediateNode>,
    ) -> anyhow::Result<HashMap<NodeHash, bool>> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn generate_code_embeddings(
        &self,
        embedding_config: EmbeddingConfig,
        fragments: Vec<full_source_code_embedding::Fragment>,
        root_hash: NodeHash,
        repo_metadata: RepoMetadata,
    ) -> anyhow::Result<HashMap<ContentHash, bool>> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn provide_negative_feedback_response_for_ai_conversation(
        &self,
        conversation_id: String,
        request_ids: Vec<String>,
    ) -> anyhow::Result<i32, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn create_agent_task(
        &self,
        prompt: String,
        environment_uid: Option<String>,
        parent_run_id: Option<String>,
        config: Option<AgentConfigSnapshot>,
    ) -> anyhow::Result<AmbientAgentTaskId, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn update_agent_task(
        &self,
        task_id: AmbientAgentTaskId,
        task_state: Option<AgentTaskState>,
        session_id: Option<session_sharing_protocol::common::SessionId>,
        conversation_id: Option<String>,
        status_message: Option<TaskStatusUpdate>,
    ) -> anyhow::Result<(), anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn rename_conversation(
        &self,
        conversation_id: String,
        title: String,
    ) -> anyhow::Result<RenameConversationResponse, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn list_agent_runs_raw(
        &self,
        limit: i32,
        filter: TaskListFilter,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_agent_run_raw(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn submit_run_followup(
        &self,
        run_id: &AmbientAgentTaskId,
        request: RunFollowupRequest,
    ) -> anyhow::Result<(), anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_scheduled_agent_history(
        &self,
        schedule_id: &str,
    ) -> anyhow::Result<ScheduledAgentHistory, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_ai_conversation(
        &self,
        server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<(ConversationData, ServerAIConversationMetadata), anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn list_ai_conversation_metadata(
        &self,
        conversation_ids: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<ServerAIConversationMetadata>> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_ai_conversation_format(
        &self,
        server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<AIAgentConversationFormat, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_block_snapshot(
        &self,
        server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<SerializedBlock, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn delete_ai_conversation(
        &self,
        server_conversation_token: String,
    ) -> anyhow::Result<(), anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn list_skills(
        &self,
        repo: Option<String>,
    ) -> anyhow::Result<Vec<AgentSkillItem>, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn list_agents(&self) -> anyhow::Result<Vec<AgentResponse>, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn list_agents_raw(&self) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_agent(&self, uid: &str) -> anyhow::Result<AgentResponse, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_agent_raw(&self, uid: &str) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn create_agent(
        &self,
        request: CreateAgentRequest,
    ) -> anyhow::Result<AgentResponse, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn create_agent_raw(
        &self,
        request: CreateAgentRequest,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn update_agent(
        &self,
        uid: &str,
        request: UpdateAgentRequest,
    ) -> anyhow::Result<AgentResponse, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn update_agent_raw(
        &self,
        uid: &str,
        request: UpdateAgentRequest,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn delete_agent(&self, uid: &str) -> anyhow::Result<(), anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn cancel_ambient_agent_task(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<(), anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_task_git_credentials(
        &self,
        task_id: String,
        workload_token: String,
    ) -> anyhow::Result<Vec<GitCredential>, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_task_attachments(
        &self,
        task_id: String,
    ) -> anyhow::Result<Vec<TaskAttachment>, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn create_file_artifact_upload_target(
        &self,
        request: CreateFileArtifactUploadRequest,
    ) -> anyhow::Result<CreateFileArtifactUploadResponse, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn confirm_file_artifact_upload(
        &self,
        artifact_uid: String,
        checksum: String,
    ) -> anyhow::Result<FileArtifactRecord, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_artifact_download(
        &self,
        artifact_uid: &str,
    ) -> anyhow::Result<ArtifactDownloadResponse, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn prepare_attachments_for_upload(
        &self,
        task_id: &AmbientAgentTaskId,
        files: &[AttachmentFileInfo],
    ) -> anyhow::Result<PrepareAttachmentUploadsResponse, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn download_task_attachments(
        &self,
        task_id: &AmbientAgentTaskId,
        attachment_ids: &[String],
    ) -> anyhow::Result<DownloadAttachmentsResponse, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_handoff_snapshot_attachments(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<Vec<TaskAttachment>, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_request_limit_info(&self) -> Result<RequestUsageInfo, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn list_connected_self_hosted_workers(
        &self,
    ) -> Result<ListConnectedSelfHostedWorkersResponse, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn spawn_agent(
        &self,
        request: SpawnAgentRequest,
    ) -> anyhow::Result<SpawnAgentResponse, anyhow::Error> {
        let _ = request;
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn upload_local_handoff_snapshot(
        &self,
        request: UploadLocalHandoffSnapshotRequest,
    ) -> anyhow::Result<UploadLocalHandoffSnapshotResponse, anyhow::Error> {
        let _ = request;
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn fork_conversation(
        &self,
        conversation_id: String,
        title: Option<String>,
    ) -> anyhow::Result<ForkConversationResponse, anyhow::Error> {
        let _ = (conversation_id, title);
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn list_ambient_agent_tasks(
        &self,
        limit: i32,
        filter: TaskListFilter,
    ) -> anyhow::Result<Vec<AmbientAgentTask>, anyhow::Error> {
        let _ = (limit, filter);
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_ambient_agent_task(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<AmbientAgentTask, anyhow::Error> {
        let _ = task_id;
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn send_agent_message(
        &self,
        request: SendAgentMessageRequest,
    ) -> anyhow::Result<SendAgentMessageResponse, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn update_event_sequence_on_server(
        &self,
        run_id: &str,
        sequence: i64,
    ) -> anyhow::Result<(), anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn report_agent_event(
        &self,
        run_id: &str,
        request: ReportAgentEventRequest,
    ) -> anyhow::Result<ReportAgentEventResponse, anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn post_agent_run_client_event(
        &self,
        run_id: &AmbientAgentTaskId,
        request: AgentRunClientEventRequest,
    ) -> anyhow::Result<(), anyhow::Error> {
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn mark_message_delivered(&self, message_id: &str) -> anyhow::Result<(), anyhow::Error> {
        let _ = message_id;
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn list_agent_messages(
        &self,
        run_id: &str,
        request: ListAgentMessagesRequest,
    ) -> anyhow::Result<Vec<AgentMessageHeader>, anyhow::Error> {
        let _ = (run_id, request);
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn read_agent_message(
        &self,
        message_id: &str,
    ) -> anyhow::Result<ReadAgentMessageResponse, anyhow::Error> {
        let _ = message_id;
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_public_conversation(
        &self,
        conversation_id: &str,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        let _ = conversation_id;
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn get_run_conversation(
        &self,
        run_id: &str,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        let _ = run_id;
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

    async fn generate_code_review_content(
        &self,
        request: GenerateCodeReviewContentRequest,
    ) -> Result<GenerateCodeReviewContentResponse, anyhow::Error> {
        let _ = request;
        Err(anyhow!("wormhole-slim: cloud AI API unavailable"))
    }

}

#[cfg(test)]
mockall::mock! {
    pub AIClient {}
    impl AIClient for AIClient {}
}