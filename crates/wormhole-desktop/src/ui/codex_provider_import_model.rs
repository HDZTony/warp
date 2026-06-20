use std::sync::{Arc, Mutex};

use wormhole_desktop_core::agent_deeplink::DeepLinkImportPreviewDto;

#[derive(Debug, Default)]
pub struct CodexProviderImportModel {
    pub preview: Option<DeepLinkImportPreviewDto>,
    pub message: String,
    pub importing: bool,
}

pub type SharedCodexProviderImportModel = Arc<Mutex<CodexProviderImportModel>>;

pub fn new_shared_import_model() -> SharedCodexProviderImportModel {
    Arc::new(Mutex::new(CodexProviderImportModel::default()))
}

pub fn open_preview(model: &SharedCodexProviderImportModel, preview: DeepLinkImportPreviewDto) {
    if let Ok(mut guard) = model.lock() {
        guard.preview = Some(preview);
        guard.message.clear();
        guard.importing = false;
    }
}

pub fn close_preview(model: &SharedCodexProviderImportModel) {
    if let Ok(mut guard) = model.lock() {
        guard.preview = None;
        guard.message.clear();
        guard.importing = false;
    }
}

pub fn set_message(model: &SharedCodexProviderImportModel, message: impl Into<String>) {
    if let Ok(mut guard) = model.lock() {
        guard.message = message.into();
    }
}

pub fn snapshot_preview(
    model: &SharedCodexProviderImportModel,
) -> Option<DeepLinkImportPreviewDto> {
    model.lock().ok().and_then(|guard| guard.preview.clone())
}

pub fn snapshot_message(model: &SharedCodexProviderImportModel) -> String {
    model
        .lock()
        .map(|guard| guard.message.clone())
        .unwrap_or_default()
}

pub fn snapshot_importing(model: &SharedCodexProviderImportModel) -> bool {
    model.lock().map(|guard| guard.importing).unwrap_or(false)
}

pub fn set_importing(model: &SharedCodexProviderImportModel, importing: bool) {
    if let Ok(mut guard) = model.lock() {
        guard.importing = importing;
    }
}
