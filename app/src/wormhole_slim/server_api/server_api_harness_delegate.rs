use async_trait::async_trait;

use super::harness_support::{
    HarnessSupportClient, ResolvePromptRequest, ResolvedHarnessPrompt, SnapshotUploadRequest,
    StubHarnessSupportClient, UploadTarget,
};
use super::slim_provider::ServerApi;
use crate::ai::agent::conversation::AIConversationId;
use crate::ai::artifacts::Artifact;

#[async_trait]
impl HarnessSupportClient for ServerApi {
    async fn create_external_conversation(&self, format: &str) -> anyhow::Result<AIConversationId> {
        StubHarnessSupportClient.create_external_conversation(format).await
    }

    async fn get_transcript_upload_target(
        &self,
        conversation_id: &AIConversationId,
    ) -> anyhow::Result<UploadTarget> {
        StubHarnessSupportClient
            .get_transcript_upload_target(conversation_id)
            .await
    }

    async fn get_block_snapshot_upload_target(
        &self,
        conversation_id: &AIConversationId,
    ) -> anyhow::Result<UploadTarget> {
        StubHarnessSupportClient
            .get_block_snapshot_upload_target(conversation_id)
            .await
    }

    async fn resolve_prompt(
        &self,
        request: ResolvePromptRequest,
    ) -> anyhow::Result<ResolvedHarnessPrompt> {
        StubHarnessSupportClient.resolve_prompt(request).await
    }

    async fn report_artifact(&self, artifact: &Artifact) -> anyhow::Result<super::harness_support::ReportArtifactResponse> {
        StubHarnessSupportClient.report_artifact(artifact).await
    }

    async fn notify_user(&self, message: &str) -> anyhow::Result<()> {
        StubHarnessSupportClient.notify_user(message).await
    }

    async fn finish_task(&self, success: bool, summary: &str) -> anyhow::Result<()> {
        StubHarnessSupportClient.finish_task(success, summary).await
    }

    async fn report_clean_shutdown(&self) -> anyhow::Result<()> {
        StubHarnessSupportClient.report_clean_shutdown().await
    }

    async fn report_error_shutdown(
        &self,
        error_category: String,
        error_message: String,
    ) -> anyhow::Result<()> {
        StubHarnessSupportClient
            .report_error_shutdown(error_category, error_message)
            .await
    }

    async fn get_snapshot_upload_targets(
        &self,
        request: &SnapshotUploadRequest,
    ) -> anyhow::Result<Vec<UploadTarget>> {
        StubHarnessSupportClient
            .get_snapshot_upload_targets(request)
            .await
    }

    async fn fetch_transcript(&self) -> anyhow::Result<bytes::Bytes> {
        StubHarnessSupportClient.fetch_transcript().await
    }

    fn http_client(&self) -> &http_client::Client {
        StubHarnessSupportClient.http_client()
    }
}
