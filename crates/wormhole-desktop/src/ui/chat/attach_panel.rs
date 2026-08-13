use std::path::{Path, PathBuf};

use storage_core::MAX_ATTACHMENT_BYTES;

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

pub fn pick_files_for_kind(kind: AttachKind) -> Vec<PathBuf> {
    match kind {
        AttachKind::Location => Vec::new(),
        AttachKind::Media => rfd::FileDialog::new()
            .set_title("选择图片或视频")
            .add_filter(
                "图片",
                &["png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "heic"],
            )
            .add_filter("视频", &["mp4", "mov", "webm", "mkv", "avi"])
            .pick_files()
            .unwrap_or_default(),
        AttachKind::Document => rfd::FileDialog::new()
            .set_title("选择文档")
            .add_filter(
                "文档",
                &[
                    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "md", "zip", "rar",
                    "7z", "csv", "json",
                ],
            )
            .add_filter(
                "图片或视频（按文件发送）",
                &[
                    "png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "heic", "mp4", "mov",
                    "webm", "mkv", "avi",
                ],
            )
            .add_filter("所有文件", &["*"])
            .pick_files()
            .unwrap_or_default(),
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
}
