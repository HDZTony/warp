use std::path::{Path, PathBuf};

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

pub fn pick_file_for_kind(kind: AttachKind) -> Option<PathBuf> {
    match kind {
        AttachKind::Location => None,
        AttachKind::Media => rfd::FileDialog::new()
            .set_title("选择图片或视频")
            .add_filter(
                "图片",
                &["png", "jpg", "jpeg", "gif", "webp", "bmp", "heic"],
            )
            .add_filter("视频", &["mp4", "mov", "webm", "mkv", "avi"])
            .pick_file(),
        AttachKind::Document => rfd::FileDialog::new()
            .set_title("选择文档")
            .add_filter(
                "文档",
                &[
                    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "md", "zip", "rar",
                    "7z", "csv", "json",
                ],
            )
            .pick_file(),
    }
}

pub fn attachment_kind_for_path(path: &Path, menu_kind: AttachKind) -> String {
    if menu_kind == AttachKind::Document {
        return "document".into();
    }
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "mp4" | "mov" | "webm" | "mkv" | "avi" => "video".into(),
        _ => "image".into(),
    }
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
    }
}
