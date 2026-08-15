use std::path::{Path, PathBuf};

use storage_core::MAX_ATTACHMENT_BYTES;
use warpui_core::platform::file_picker::{FilePickerConfiguration, FileType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachKind {
    Media,
    Document,
    Location,
}

impl AttachKind {
    pub fn label(self) -> &'static str {
        match self {
            AttachKind::Media => "图片或视频",
            AttachKind::Document => "文档",
            AttachKind::Location => "位置",
        }
    }

    pub fn desc(self) -> &'static str {
        match self {
            AttachKind::Media => "从相册或终端共享选择",
            AttachKind::Document => "PDF、Office、压缩包等",
            AttachKind::Location => "发送位置（即将支持）",
        }
    }

    pub fn icon_path(self) -> &'static str {
        match self {
            AttachKind::Media => "chat-attach-media.svg",
            AttachKind::Document => "chat-attach-document.svg",
            AttachKind::Location => "chat-attach-location.svg",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedStagedFile {
    pub path: PathBuf,
    pub kind: String,
    pub name: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageError {
    Directory { name: String },
    Missing { name: String },
    Unreadable { name: String },
    TooLarge { name: String, size: u64 },
}

impl StageError {
    pub fn message(&self) -> String {
        match self {
            StageError::Directory { .. } => wormhole_i18n::t("chat.attachment.directory_unsupported"),
            StageError::Missing { name } | StageError::Unreadable { name } => {
                wormhole_i18n::t_args("chat.attachment.unreadable", &[("name", name)])
            }
            StageError::TooLarge { name, .. } => {
                wormhole_i18n::t_args("chat.attachment.too_large", &[("name", name)])
            }
        }
    }
}

/// Native picker config for an attach-menu kind.
///
/// Uses Warp `open_file_picker` (macOS `beginWithCompletionHandler`) instead of
/// `rfd` on a blocking thread, which deadlocks `NSOpenPanel` against the UI
/// runloop. Empty `file_types` means all files (document menu).
pub fn picker_config_for_kind(kind: AttachKind) -> Option<FilePickerConfiguration> {
    match kind {
        AttachKind::Location => None,
        AttachKind::Media => Some(
            FilePickerConfiguration::new()
                .allow_multi_select()
                .set_allowed_file_types(vec![FileType::Image, FileType::Movie]),
        ),
        AttachKind::Document => Some(FilePickerConfiguration::new().allow_multi_select()),
    }
}

pub fn path_extension_lower(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

pub fn is_video_extension(ext: &str) -> bool {
    matches!(ext, "mp4" | "mov" | "webm" | "mkv" | "avi")
}

pub fn is_image_extension(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "bmp" | "heic" | "heif"
    )
}

pub fn attachment_kind_for_path(path: &Path, menu_kind: AttachKind) -> String {
    if menu_kind == AttachKind::Document {
        return "document".into();
    }
    attachment_kind_for_drop_path(path)
}

/// Telegram drop/paste: images and videos send as media; everything else as a file.
pub fn attachment_kind_for_drop_path(path: &Path) -> String {
    let ext = path_extension_lower(path);
    if is_video_extension(&ext) {
        "video".into()
    } else if is_image_extension(&ext) {
        "image".into()
    } else {
        "document".into()
    }
}

pub fn exceeds_attachment_limit(size: u64) -> bool {
    size > MAX_ATTACHMENT_BYTES
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| wormhole_i18n::t("chat.attachment.default_name"))
}

pub fn prepare_staged_file(path: PathBuf, menu_kind: AttachKind) -> Result<PreparedStagedFile, StageError> {
    let kind = attachment_kind_for_path(&path, menu_kind);
    prepare_staged_file_with_kind(path, kind)
}

pub fn prepare_dropped_file(path: PathBuf) -> Result<PreparedStagedFile, StageError> {
    let kind = attachment_kind_for_drop_path(&path);
    prepare_staged_file_with_kind(path, kind)
}

pub fn prepare_staged_file_with_kind(
    path: PathBuf,
    kind: String,
) -> Result<PreparedStagedFile, StageError> {
    let name = display_name(&path);
    if !path.exists() {
        return Err(StageError::Missing { name });
    }
    if path.is_dir() {
        return Err(StageError::Directory { name });
    }
    let size = std::fs::metadata(&path)
        .map(|meta| meta.len())
        .map_err(|_| StageError::Unreadable { name: name.clone() })?;
    if exceeds_attachment_limit(size) {
        return Err(StageError::TooLarge { name, size });
    }
    Ok(PreparedStagedFile {
        path,
        kind,
        name,
        size,
    })
}

/// Effective attachment kind for the media-upload dialog send path.
pub fn effective_upload_kind(path: &Path, send_as_files: bool) -> String {
    if send_as_files {
        "document".into()
    } else {
        attachment_kind_for_drop_path(path)
    }
}

/// One outbound message batch from the upload dialog (caption + attachments).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadSendBatch {
    pub body: String,
    pub attachments: Vec<(String, PathBuf)>,
}

/// Build send batches: grouped = one message; ungrouped = N messages with caption on first.
pub fn build_upload_send_batches(
    items: &[(PathBuf, String)],
    caption: &str,
    group_items: bool,
    send_as_files: bool,
) -> Vec<UploadSendBatch> {
    let caption = caption.trim();
    let resolved: Vec<(String, PathBuf)> = items
        .iter()
        .map(|(path, _name)| (effective_upload_kind(path, send_as_files), path.clone()))
        .collect();
    if resolved.is_empty() {
        if caption.is_empty() {
            return Vec::new();
        }
        return vec![UploadSendBatch {
            body: caption.to_string(),
            attachments: Vec::new(),
        }];
    }
    if group_items || resolved.len() == 1 {
        return vec![UploadSendBatch {
            body: caption.to_string(),
            attachments: resolved,
        }];
    }
    resolved
        .into_iter()
        .enumerate()
        .map(|(idx, att)| UploadSendBatch {
            body: if idx == 0 {
                caption.to_string()
            } else {
                String::new()
            },
            attachments: vec![att],
        })
        .collect()
}

/// Telegram `PreparedList::hasGroupOption` (no slowmode): need at least two items.
pub fn show_media_upload_group_option(item_count: usize) -> bool {
    item_count >= 2
}

/// Telegram `hasSendImagesAsPhotosOption`: show when any image/video is present.
pub fn show_media_upload_as_file_option<'a>(
    kinds: impl IntoIterator<Item = &'a str>,
) -> bool {
    kinds.into_iter().any(|k| k == "image" || k == "video")
}

/// Telegram Remember: visible only after Group / as-document differs from open-time values.
pub fn show_media_upload_remember_option(
    group: bool,
    as_file: bool,
    initial_group: bool,
    initial_as_file: bool,
) -> bool {
    group != initial_group || as_file != initial_as_file
}

/// Whether Remember should persist the as-file flag (skip hidden / forced-unchanged).
pub fn should_persist_media_upload_as_file(
    show_as_file: bool,
    forced_as_file: bool,
    send_as_files: bool,
    initial_send_as_files: bool,
) -> bool {
    if !show_as_file {
        return false;
    }
    if forced_as_file && send_as_files == initial_send_as_files {
        return false;
    }
    true
}

/// When `WORMHOLE_SIM_USE_ATTACH_PATH` lists existing file path(s), skip the OS picker.
/// Separate multiple paths with `;` (e.g. `a.png;b.png`).
pub fn sim_use_attach_paths_from_env() -> Vec<PathBuf> {
    let Ok(raw) = std::env::var("WORMHOLE_SIM_USE_ATTACH_PATH") else {
        return Vec::new();
    };
    raw.split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_kind_desc_labels() {
        assert_eq!(AttachKind::Media.desc(), "从相册或终端共享选择");
        assert_eq!(AttachKind::Document.desc(), "PDF、Office、压缩包等");
        assert_eq!(AttachKind::Location.desc(), "发送位置（即将支持）");
    }

    #[test]
    fn media_picker_allows_images_movies_and_multi_select() {
        let config = picker_config_for_kind(AttachKind::Media).expect("media has a picker");
        assert!(config.allows_multi_select());
        assert_eq!(
            config.file_types(),
            &vec![FileType::Image, FileType::Movie]
        );
    }

    #[test]
    fn document_picker_allows_all_files() {
        let config = picker_config_for_kind(AttachKind::Document).expect("document has a picker");
        assert!(config.allows_multi_select());
        assert!(config.file_types().is_empty());
    }

    #[test]
    fn location_has_no_picker() {
        assert!(picker_config_for_kind(AttachKind::Location).is_none());
    }

    #[test]
    fn attachment_kind_for_path_detects_video() {
        assert_eq!(
            attachment_kind_for_path(Path::new("clip.mp4"), AttachKind::Media),
            "video"
        );
    }

    #[test]
    fn attachment_kind_for_path_defaults_to_image() {
        assert_eq!(
            attachment_kind_for_path(Path::new("photo.png"), AttachKind::Media),
            "image"
        );
    }

    #[test]
    fn attachment_kind_for_path_document_menu() {
        assert_eq!(
            attachment_kind_for_path(Path::new("readme.mp4"), AttachKind::Document),
            "document"
        );
        assert_eq!(
            attachment_kind_for_path(Path::new("photo.png"), AttachKind::Document),
            "document"
        );
    }

    #[test]
    fn drop_path_routes_media_and_files() {
        assert_eq!(attachment_kind_for_drop_path(Path::new("a.png")), "image");
        assert_eq!(attachment_kind_for_drop_path(Path::new("a.HEIC")), "image");
        assert_eq!(attachment_kind_for_drop_path(Path::new("a.mov")), "video");
        assert_eq!(attachment_kind_for_drop_path(Path::new("notes.pdf")), "document");
    }

    #[test]
    fn prepare_staged_file_rejects_directory() {
        let dir = tempfile::tempdir().unwrap();
        let err = prepare_dropped_file(dir.path().to_path_buf()).unwrap_err();
        assert!(matches!(err, StageError::Directory { .. }));
    }

    #[test]
    fn prepare_staged_file_rejects_missing() {
        let err = prepare_dropped_file(PathBuf::from("/no/such/chat-attach-file.png")).unwrap_err();
        assert!(matches!(err, StageError::Missing { .. }));
    }

    #[test]
    fn prepare_staged_file_accepts_small_png() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.png");
        std::fs::write(&path, [0x89, b'P', b'N', b'G']).unwrap();
        let staged = prepare_staged_file(path.clone(), AttachKind::Media).unwrap();
        assert_eq!(staged.kind, "image");
        assert_eq!(staged.name, "photo.png");
        assert_eq!(staged.size, 4);
    }

    #[test]
    fn prepare_staged_file_document_keeps_png_as_document() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.png");
        std::fs::write(&path, [0x89, b'P', b'N', b'G']).unwrap();
        let staged = prepare_staged_file(path, AttachKind::Document).unwrap();
        assert_eq!(staged.kind, "document");
    }

    #[test]
    fn prepare_multiple_files_keep_order_and_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let photo = dir.path().join("a.png");
        let clip = dir.path().join("b.mp4");
        std::fs::write(&photo, [0x89, b'P', b'N', b'G']).unwrap();
        std::fs::write(&clip, b"mp4").unwrap();
        let files = [
            prepare_staged_file(photo, AttachKind::Media).unwrap(),
            prepare_staged_file(clip, AttachKind::Media).unwrap(),
        ];
        assert_eq!(
            files
                .iter()
                .map(|file| file.kind.as_str())
                .collect::<Vec<_>>(),
            ["image", "video"]
        );
    }

    #[test]
    fn prepare_staged_file_rejects_oversize() {
        assert!(!exceeds_attachment_limit(MAX_ATTACHMENT_BYTES));
        assert!(exceeds_attachment_limit(MAX_ATTACHMENT_BYTES + 1));
        let err = StageError::TooLarge {
            name: "huge.bin".into(),
            size: MAX_ATTACHMENT_BYTES + 1,
        };
        let message = err.message();
        assert!(
            message.contains("huge.bin"),
            "oversize error should name the file: {message}"
        );
    }

    #[test]
    fn effective_upload_kind_respects_send_as_files() {
        assert_eq!(
            effective_upload_kind(Path::new("a.png"), true),
            "document"
        );
        assert_eq!(effective_upload_kind(Path::new("a.png"), false), "image");
        assert_eq!(effective_upload_kind(Path::new("a.mp4"), false), "video");
    }

    #[test]
    fn build_upload_send_batches_group_and_ungroup() {
        let items = [
            (PathBuf::from("a.png"), "a.png".into()),
            (PathBuf::from("b.png"), "b.png".into()),
        ];
        let grouped = build_upload_send_batches(&items, "hi", true, false);
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].body, "hi");
        assert_eq!(grouped[0].attachments.len(), 2);

        let ungrouped = build_upload_send_batches(&items, "hi", false, false);
        assert_eq!(ungrouped.len(), 2);
        assert_eq!(ungrouped[0].body, "hi");
        assert!(ungrouped[1].body.is_empty());
        assert_eq!(ungrouped[0].attachments.len(), 1);
        assert_eq!(ungrouped[1].attachments.len(), 1);
    }

    #[test]
    fn build_upload_send_batches_as_files() {
        let items = [(PathBuf::from("a.png"), "a.png".into())];
        let batches = build_upload_send_batches(&items, "", true, true);
        assert_eq!(batches[0].attachments[0].0, "document");
    }

    #[test]
    fn media_upload_option_visibility_matches_telegram_rules() {
        assert!(!show_media_upload_group_option(0));
        assert!(!show_media_upload_group_option(1));
        assert!(show_media_upload_group_option(2));

        assert!(!show_media_upload_as_file_option(["document"]));
        assert!(show_media_upload_as_file_option(["image"]));
        assert!(show_media_upload_as_file_option(["video", "document"]));

        assert!(!show_media_upload_remember_option(true, false, true, false));
        assert!(show_media_upload_remember_option(false, false, true, false));
        assert!(show_media_upload_remember_option(true, true, true, false));

        assert!(!should_persist_media_upload_as_file(false, false, true, false));
        assert!(!should_persist_media_upload_as_file(true, true, true, true));
        assert!(should_persist_media_upload_as_file(true, true, false, true));
        assert!(should_persist_media_upload_as_file(true, false, true, false));
    }
}
