use thiserror::Error;
use warp_core::errors::{register_error, ErrorExt};

use super::harness_support::UploadTarget;
use crate::server::server_api::ai::FileArtifactUploadTargetInfo;

#[derive(Debug, Error)]
#[error("HTTP request failed with status {status}: {body}")]
pub struct HttpStatusError {
    pub status: u16,
    pub body: String,
}

impl ErrorExt for HttpStatusError {
    fn is_actionable(&self) -> bool {
        !matches!(self.status, 408 | 429)
    }
}
register_error!(HttpStatusError);

#[cfg(feature = "local_fs")]
#[derive(Debug, Clone)]
pub struct FileUploadBody {
    path: std::path::PathBuf,
}

#[cfg(feature = "local_fs")]
impl FileUploadBody {
    pub fn new(path: std::path::PathBuf) -> Self {
        Self { path }
    }
}

#[cfg(feature = "local_fs")]
pub(crate) async fn upload_file_to_target(
    _http_client: &http_client::Client,
    _target: &FileArtifactUploadTargetInfo,
    _body: FileUploadBody,
) -> anyhow::Result<String> {
    anyhow::bail!("wormhole-slim: artifact upload is unavailable")
}

pub(crate) async fn upload_to_target<T>(
    _http_client: &http_client::Client,
    _target: &UploadTarget,
    _body: T,
) -> anyhow::Result<()> {
    Ok(())
}
