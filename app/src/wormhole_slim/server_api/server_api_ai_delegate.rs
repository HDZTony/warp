use async_trait::async_trait;
use std::collections::HashMap;

use anyhow::Result;
use chrono::{DateTime, Utc};

use super::ai::*;
use super::slim_provider::ServerApi;
use crate::ai::generate_code_review_content::api::{
    GenerateCodeReviewContentRequest, GenerateCodeReviewContentResponse,
};
use crate::ai::RequestUsageInfo;
#[async_trait]
impl AIClient for ServerApi {
    async fn generate_commands_from_natural_language(
        &self,
        prompt: String,
        ai_execution_context: Option<WarpAiExecutionContext>,
    ) -> Result<Vec<AIGeneratedCommand>, GenerateCommandsFromNaturalLanguageError> {
        StubAIClient.generate_commands_from_natural_language(prompt, ai_execution_context).await
    }

    async fn generate_dialogue_answer(
        &self,
        transcript: Vec<TranscriptPart>,
        prompt: String,
        ai_execution_context: Option<WarpAiExecutionContext>,
    ) -> anyhow::Result<GenerateDialogueResult> {
        StubAIClient.generate_dialogue_answer(transcript, prompt, ai_execution_context).await
    }

    async fn generate_metadata_for_command(
        &self,
        command: String,
    ) -> Result<GeneratedCommandMetadata, GeneratedCommandMetadataError> {
        StubAIClient.generate_metadata_for_command(command).await
    }

    async fn get_conversation_usage_history(
        &self,
        days: Option<i32>,
        limit: Option<i32>,
        last_updated_end_timestamp: Option<DateTime<Utc>>,
    ) -> Result<Vec<ConversationUsage>, anyhow::Error> {
        StubAIClient.get_conversation_usage_history(days, limit, last_updated_end_timestamp).await
    }

    async fn get_feature_model_choices(&self) -> Result<ModelsByFeature, anyhow::Error> {
        StubAIClient.get_feature_model_choices().await
    }

    async fn get_available_harnesses(&self) -> Result<Vec<HarnessAvailability>, anyhow::Error> {
        StubAIClient.get_available_harnesses().await
    }

    async fn get_free_available_models(
        &self,
        referrer: Option<String>,
    ) -> Result<ModelsByFeature, anyhow::Error> {
        StubAIClient.get_free_available_models(referrer).await
    }

    async fn update_merkle_tree(
        &self,
        embedding_config: EmbeddingConfig,
        nodes: Vec<IntermediateNode>,
    ) -> anyhow::Result<HashMap<NodeHash, bool>> {
        StubAIClient.update_merkle_tree(embedding_config, nodes).await
    }

    async fn generate_code_embeddings(
        &self,
        embedding_config: EmbeddingConfig,
        fragments: Vec<full_source_code_embedding::Fragment>,
        root_hash: NodeHash,
        repo_metadata: RepoMetadata,
    ) -> anyhow::Result<HashMap<ContentHash, bool>> {
        StubAIClient.generate_code_embeddings(embedding_config, fragments, root_hash, repo_metadata).await
    }

    async fn provide_negative_feedback_response_for_ai_conversation(
        &self,
        conversation_id: String,
        request_ids: Vec<String>,
    ) -> anyhow::Result<i32, anyhow::Error> {
        StubAIClient.provide_negative_feedback_response_for_ai_conversation(conversation_id, request_ids).await
    }

    async fn create_agent_task(
        &self,
        prompt: String,
        environment_uid: Option<String>,
        parent_run_id: Option<String>,
        config: Option<AgentConfigSnapshot>,
    ) -> anyhow::Result<AmbientAgentTaskId, anyhow::Error> {
        StubAIClient.create_agent_task(prompt, environment_uid, parent_run_id, config).await
    }

    async fn update_agent_task(
        &self,
        task_id: AmbientAgentTaskId,
        task_state: Option<AgentTaskState>,
        session_id: Option<session_sharing_protocol::common::SessionId>,
        conversation_id: Option<String>,
        status_message: Option<TaskStatusUpdate>,
    ) -> anyhow::Result<(), anyhow::Error> {
        StubAIClient.update_agent_task(task_id, task_state, session_id, conversation_id, status_message).await
    }

    async fn rename_conversation(
        &self,
        conversation_id: String,
        title: String,
    ) -> anyhow::Result<RenameConversationResponse, anyhow::Error> {
        StubAIClient.rename_conversation(conversation_id, title).await
    }

    async fn list_agent_runs_raw(
        &self,
        limit: i32,
        filter: TaskListFilter,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        StubAIClient.list_agent_runs_raw(limit, filter).await
    }

    async fn get_agent_run_raw(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        StubAIClient.get_agent_run_raw(task_id).await
    }

    async fn submit_run_followup(
        &self,
        run_id: &AmbientAgentTaskId,
        request: RunFollowupRequest,
    ) -> anyhow::Result<(), anyhow::Error> {
        StubAIClient.submit_run_followup(run_id, request).await
    }

    async fn get_scheduled_agent_history(
        &self,
        schedule_id: &str,
    ) -> anyhow::Result<ScheduledAgentHistory, anyhow::Error> {
        StubAIClient.get_scheduled_agent_history(schedule_id).await
    }

    async fn get_ai_conversation(
        &self,
        server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<(ConversationData, ServerAIConversationMetadata), anyhow::Error> {
        StubAIClient.get_ai_conversation(server_conversation_token).await
    }

    async fn list_ai_conversation_metadata(
        &self,
        conversation_ids: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<ServerAIConversationMetadata>> {
        StubAIClient.list_ai_conversation_metadata(conversation_ids).await
    }

    async fn get_ai_conversation_format(
        &self,
        server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<AIAgentConversationFormat, anyhow::Error> {
        StubAIClient.get_ai_conversation_format(server_conversation_token).await
    }

    async fn get_block_snapshot(
        &self,
        server_conversation_token: ServerConversationToken,
    ) -> anyhow::Result<SerializedBlock, anyhow::Error> {
        StubAIClient.get_block_snapshot(server_conversation_token).await
    }

    async fn delete_ai_conversation(
        &self,
        server_conversation_token: String,
    ) -> anyhow::Result<(), anyhow::Error> {
        StubAIClient.delete_ai_conversation(server_conversation_token).await
    }

    async fn list_skills(
        &self,
        repo: Option<String>,
    ) -> anyhow::Result<Vec<AgentSkillItem>, anyhow::Error> {
        StubAIClient.list_skills(repo).await
    }

    async fn list_agents(&self) -> anyhow::Result<Vec<AgentResponse>, anyhow::Error> {
        StubAIClient.list_agents().await
    }

    async fn list_agents_raw(&self) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        StubAIClient.list_agents_raw().await
    }

    async fn get_agent(&self, uid: &str) -> anyhow::Result<AgentResponse, anyhow::Error> {
        StubAIClient.get_agent(uid).await
    }

    async fn get_agent_raw(&self, uid: &str) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        StubAIClient.get_agent_raw(uid).await
    }

    async fn create_agent(
        &self,
        request: CreateAgentRequest,
    ) -> anyhow::Result<AgentResponse, anyhow::Error> {
        StubAIClient.create_agent(request).await
    }

    async fn create_agent_raw(
        &self,
        request: CreateAgentRequest,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        StubAIClient.create_agent_raw(request).await
    }

    async fn update_agent(
        &self,
        uid: &str,
        request: UpdateAgentRequest,
    ) -> anyhow::Result<AgentResponse, anyhow::Error> {
        StubAIClient.update_agent(uid, request).await
    }

    async fn update_agent_raw(
        &self,
        uid: &str,
        request: UpdateAgentRequest,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        StubAIClient.update_agent_raw(uid, request).await
    }

    async fn delete_agent(&self, uid: &str) -> anyhow::Result<(), anyhow::Error> {
        StubAIClient.delete_agent(uid).await
    }

    async fn cancel_ambient_agent_task(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<(), anyhow::Error> {
        StubAIClient.cancel_ambient_agent_task(task_id).await
    }

    async fn get_task_git_credentials(
        &self,
        task_id: String,
        workload_token: String,
    ) -> anyhow::Result<Vec<GitCredential>, anyhow::Error> {
        StubAIClient.get_task_git_credentials(task_id, workload_token).await
    }

    async fn get_task_attachments(
        &self,
        task_id: String,
    ) -> anyhow::Result<Vec<TaskAttachment>, anyhow::Error> {
        StubAIClient.get_task_attachments(task_id).await
    }

    async fn create_file_artifact_upload_target(
        &self,
        request: CreateFileArtifactUploadRequest,
    ) -> anyhow::Result<CreateFileArtifactUploadResponse, anyhow::Error> {
        StubAIClient.create_file_artifact_upload_target(request).await
    }

    async fn confirm_file_artifact_upload(
        &self,
        artifact_uid: String,
        checksum: String,
    ) -> anyhow::Result<FileArtifactRecord, anyhow::Error> {
        StubAIClient.confirm_file_artifact_upload(artifact_uid, checksum).await
    }

    async fn get_artifact_download(
        &self,
        artifact_uid: &str,
    ) -> anyhow::Result<ArtifactDownloadResponse, anyhow::Error> {
        StubAIClient.get_artifact_download(artifact_uid).await
    }

    async fn prepare_attachments_for_upload(
        &self,
        task_id: &AmbientAgentTaskId,
        files: &[AttachmentFileInfo],
    ) -> anyhow::Result<PrepareAttachmentUploadsResponse, anyhow::Error> {
        StubAIClient.prepare_attachments_for_upload(task_id, files).await
    }

    async fn download_task_attachments(
        &self,
        task_id: &AmbientAgentTaskId,
        attachment_ids: &[String],
    ) -> anyhow::Result<DownloadAttachmentsResponse, anyhow::Error> {
        StubAIClient.download_task_attachments(task_id, attachment_ids).await
    }

    async fn get_handoff_snapshot_attachments(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<Vec<TaskAttachment>, anyhow::Error> {
        StubAIClient.get_handoff_snapshot_attachments(task_id).await
    }

    async fn get_request_limit_info(&self) -> Result<RequestUsageInfo, anyhow::Error> {
        StubAIClient.get_request_limit_info().await
    }

    async fn list_connected_self_hosted_workers(
        &self,
    ) -> Result<ListConnectedSelfHostedWorkersResponse, anyhow::Error> {
        StubAIClient.list_connected_self_hosted_workers().await
    }

    async fn spawn_agent(
        &self,
        request: SpawnAgentRequest,
    ) -> anyhow::Result<SpawnAgentResponse, anyhow::Error> {
        StubAIClient.spawn_agent(request).await
    }

    async fn upload_local_handoff_snapshot(
        &self,
        request: UploadLocalHandoffSnapshotRequest,
    ) -> anyhow::Result<UploadLocalHandoffSnapshotResponse, anyhow::Error> {
        StubAIClient.upload_local_handoff_snapshot(request).await
    }

    async fn fork_conversation(
        &self,
        conversation_id: String,
        title: Option<String>,
    ) -> anyhow::Result<ForkConversationResponse, anyhow::Error> {
        StubAIClient.fork_conversation(conversation_id, title).await
    }

    async fn list_ambient_agent_tasks(
        &self,
        limit: i32,
        filter: TaskListFilter,
    ) -> anyhow::Result<Vec<AmbientAgentTask>, anyhow::Error> {
        StubAIClient.list_ambient_agent_tasks(limit, filter).await
    }

    async fn get_ambient_agent_task(
        &self,
        task_id: &AmbientAgentTaskId,
    ) -> anyhow::Result<AmbientAgentTask, anyhow::Error> {
        StubAIClient.get_ambient_agent_task(task_id).await
    }

    async fn send_agent_message(
        &self,
        request: SendAgentMessageRequest,
    ) -> anyhow::Result<SendAgentMessageResponse, anyhow::Error> {
        StubAIClient.send_agent_message(request).await
    }

    async fn update_event_sequence_on_server(
        &self,
        run_id: &str,
        sequence: i64,
    ) -> anyhow::Result<(), anyhow::Error> {
        StubAIClient.update_event_sequence_on_server(run_id, sequence).await
    }

    async fn report_agent_event(
        &self,
        run_id: &str,
        request: ReportAgentEventRequest,
    ) -> anyhow::Result<ReportAgentEventResponse, anyhow::Error> {
        StubAIClient.report_agent_event(run_id, request).await
    }

    async fn post_agent_run_client_event(
        &self,
        run_id: &AmbientAgentTaskId,
        request: AgentRunClientEventRequest,
    ) -> anyhow::Result<(), anyhow::Error> {
        StubAIClient.post_agent_run_client_event(run_id, request).await
    }

    async fn mark_message_delivered(&self, message_id: &str) -> anyhow::Result<(), anyhow::Error> {
        StubAIClient.mark_message_delivered(message_id).await
    }

    async fn list_agent_messages(
        &self,
        run_id: &str,
        request: ListAgentMessagesRequest,
    ) -> anyhow::Result<Vec<AgentMessageHeader>, anyhow::Error> {
        StubAIClient.list_agent_messages(run_id, request).await
    }

    async fn read_agent_message(
        &self,
        message_id: &str,
    ) -> anyhow::Result<ReadAgentMessageResponse, anyhow::Error> {
        StubAIClient.read_agent_message(message_id).await
    }

    async fn get_public_conversation(
        &self,
        conversation_id: &str,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        StubAIClient.get_public_conversation(conversation_id).await
    }

    async fn get_run_conversation(
        &self,
        run_id: &str,
    ) -> anyhow::Result<serde_json::Value, anyhow::Error> {
        StubAIClient.get_run_conversation(run_id).await
    }

    async fn generate_code_review_content(
        &self,
        request: GenerateCodeReviewContentRequest,
    ) -> Result<GenerateCodeReviewContentResponse, anyhow::Error> {
        StubAIClient.generate_code_review_content(request).await
    }

}