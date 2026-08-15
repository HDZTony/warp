pub fn write_clipboard_text(text: &str) -> Result<(), String> {
    #[cfg(windows)]
    if let Ok(()) = write_clipboard_text_win32(text) {
        return Ok(());
    }
    arboard::Clipboard::new()
        .map_err(|e| e.to_string())?
        .set_text(text)
        .map_err(|e| e.to_string())
}

pub fn read_clipboard_text() -> Option<String> {
    #[cfg(windows)]
    if let Some(text) = read_clipboard_text_win32() {
        return Some(text);
    }
    arboard::Clipboard::new()
        .ok()?
        .get_text()
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(windows)]
fn write_clipboard_text_win32(text: &str) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

    let wide: Vec<u16> = OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let byte_len = wide.len() * std::mem::size_of::<u16>();

    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err("无法打开剪贴板".into());
        }

        let mut handle: HANDLE = std::ptr::null_mut();
        let result = (|| {
            if EmptyClipboard() == 0 {
                return Err("无法清空剪贴板".into());
            }
            handle = GlobalAlloc(GMEM_MOVEABLE, byte_len);
            if handle.is_null() {
                return Err("无法分配剪贴板内存".into());
            }
            let ptr = GlobalLock(handle) as *mut u16;
            if ptr.is_null() {
                return Err("无法锁定剪贴板内存".into());
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
            GlobalUnlock(handle);
            if SetClipboardData(CF_UNICODETEXT as u32, handle).is_null() {
                return Err("无法写入剪贴板".into());
            }
            handle = std::ptr::null_mut();
            Ok(())
        })();

        if !handle.is_null() {
            use windows_sys::Win32::Foundation::GlobalFree;
            GlobalFree(handle);
        }
        CloseClipboard();
        result
    }
}

#[cfg(windows)]
fn read_clipboard_text_win32() -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT as u32) == 0 {
            return None;
        }
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT as u32);
        if handle.is_null() {
            CloseClipboard();
            return None;
        }
        let ptr = GlobalLock(handle) as *const u16;
        if ptr.is_null() {
            CloseClipboard();
            return None;
        }
        let mut len = 0usize;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr, len);
        let text = OsString::from_wide(slice).to_string_lossy().into_owned();
        GlobalUnlock(handle);
        CloseClipboard();
        let text = text.trim().to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

pub fn read_clipboard_image_png() -> Option<(Vec<u8>, String)> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let image = clipboard.get_image().ok()?;
    if image.width == 0 || image.height == 0 {
        return None;
    }
    let width = u32::try_from(image.width).ok()?;
    let height = u32::try_from(image.height).ok()?;
    let buffer = image::RgbaImage::from_raw(width, height, image.bytes.into_owned())?;
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(buffer)
        .write_to(
            &mut std::io::Cursor::new(&mut png),
            image::ImageFormat::Png,
        )
        .ok()?;
    if png.is_empty() {
        return None;
    }
    Some((
        png,
        wormhole_i18n::t("chat.attachment.clipboard_image"),
    ))
}

/// Copy an image file onto the system clipboard as RGBA pixels.
pub fn write_clipboard_image_from_path(path: &std::path::Path) -> Result<(), String> {
    let rgb = storage_core::decode_chat_image_to_rgb(path).map_err(|err| err.to_string())?;
    let (width, height) = rgb.dimensions();
    let rgba = image::DynamicImage::ImageRgb8(rgb).into_rgba8();
    let mut clipboard = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clipboard
        .set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Owned(rgba.into_raw()),
        })
        .map_err(|e| e.to_string())
}

/// Copy one or more file paths onto the system clipboard so they can be pasted
/// as files (Explorer / Finder / file managers).
pub fn write_clipboard_files(paths: &[std::path::PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Err("没有可复制的文件".into());
    }
    for path in paths {
        if !path.exists() {
            return Err(format!("文件不存在: {}", path.display()));
        }
    }
    #[cfg(windows)]
    {
        return write_clipboard_files_win32(paths);
    }
    #[cfg(not(windows))]
    {
        // Cross-platform fallback: `file://` URI list as plain text. Native
        // CF_HDROP / Finder pasteboard file lists need platform APIs; URI text
        // still lets compose paste and many apps resolve the path.
        let text = paths
            .iter()
            .map(|path| {
                let absolute = path
                    .canonicalize()
                    .unwrap_or_else(|_| path.to_path_buf());
                format!("file://{}", absolute.display())
            })
            .collect::<Vec<_>>()
            .join("\n");
        write_clipboard_text(&text)
    }
}

#[cfg(windows)]
fn write_clipboard_files_win32(paths: &[std::path::PathBuf]) -> Result<(), String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };

    /// Win32 DROPFILES header (see shellapi.h). Kept local so we do not pull
    /// `Win32_UI_Shell` into wormhole-desktop solely for this struct.
    #[repr(C)]
    struct DropFiles {
        p_files: u32,
        pt_x: i32,
        pt_y: i32,
        f_nc: i32,
        f_wide: i32,
    }

    // Build double-null-terminated wide path list.
    let mut wide: Vec<u16> = Vec::new();
    for path in paths {
        let absolute = path
            .canonicalize()
            .unwrap_or_else(|_| path.to_path_buf());
        wide.extend(OsStr::new(&absolute).encode_wide());
        wide.push(0);
    }
    wide.push(0);

    let header_size = std::mem::size_of::<DropFiles>();
    let path_bytes = wide.len() * std::mem::size_of::<u16>();
    let total = header_size + path_bytes;

    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return Err("无法打开剪贴板".into());
        }

        let mut handle: HANDLE = std::ptr::null_mut();
        let result = (|| {
            if EmptyClipboard() == 0 {
                return Err("无法清空剪贴板".into());
            }
            handle = GlobalAlloc(GMEM_MOVEABLE, total);
            if handle.is_null() {
                return Err("无法分配剪贴板内存".into());
            }
            let ptr = GlobalLock(handle) as *mut u8;
            if ptr.is_null() {
                return Err("无法锁定剪贴板内存".into());
            }
            std::ptr::write_bytes(ptr, 0, total);
            let dropfiles = ptr as *mut DropFiles;
            (*dropfiles).p_files = header_size as u32;
            (*dropfiles).f_wide = 1; // TRUE — Unicode paths
            let dest = ptr.add(header_size) as *mut u16;
            std::ptr::copy_nonoverlapping(wide.as_ptr(), dest, wide.len());
            GlobalUnlock(handle);

            // CF_HDROP is clipboard format 15.
            const CF_HDROP: u32 = 15;
            if SetClipboardData(CF_HDROP, handle).is_null() {
                return Err("无法写入文件剪贴板".into());
            }
            handle = std::ptr::null_mut();
            Ok(())
        })();

        if !handle.is_null() {
            use windows_sys::Win32::Foundation::GlobalFree;
            GlobalFree(handle);
        }
        CloseClipboard();
        result
    }
}

pub fn clipboard_image_kind_and_ext(mime: &str) -> (&'static str, &'static str) {
    match mime {
        "image/png" => ("image", "png"),
        "image/jpeg" | "image/jpg" => ("image", "jpg"),
        "image/gif" => ("image", "gif"),
        "image/webp" => ("image", "webp"),
        "image/bmp" => ("image", "bmp"),
        "image/heic" | "image/heif" => ("image", "heic"),
        "image/avif" => ("image", "avif"),
        _ => ("document", "bin"),
    }
}

pub fn paths_from_clipboard_text(text: &str) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim().trim_matches('"');
        if trimmed.is_empty() {
            continue;
        }
        let path = trimmed
            .strip_prefix("file://")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(trimmed));
        if path.exists() {
            paths.push(path);
        }
    }
    paths
}

pub fn paths_from_clipboard_content(
    content: &warpui_core::clipboard::ClipboardContent,
) -> Vec<std::path::PathBuf> {
    if let Some(paths) = &content.paths {
        let existing = paths
            .iter()
            .map(std::path::PathBuf::from)
            .filter(|path| path.exists())
            .collect::<Vec<_>>();
        if !existing.is_empty() {
            return existing;
        }
    }
    paths_from_clipboard_text(&content.plain_text)
}

pub fn read_clipboard_existing_paths() -> Vec<std::path::PathBuf> {
    let text = match read_clipboard_text() {
        Some(text) => text,
        None => return Vec::new(),
    };
    paths_from_clipboard_text(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_image_kind_routes_raster_and_unknown() {
        assert_eq!(clipboard_image_kind_and_ext("image/png"), ("image", "png"));
        assert_eq!(clipboard_image_kind_and_ext("image/jpeg"), ("image", "jpg"));
        assert_eq!(
            clipboard_image_kind_and_ext("image/svg+xml"),
            ("document", "bin")
        );
    }

    #[test]
    fn paths_from_clipboard_text_keeps_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("photo.png");
        std::fs::write(&file, b"png").unwrap();
        let missing = dir.path().join("gone.png");
        let text = format!("file://{}\n\"{}\"\n{}", file.display(), file.display(), missing.display());
        let paths = paths_from_clipboard_text(&text);
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().all(|path| path.ends_with("photo.png")));
    }

    #[test]
    fn paths_from_clipboard_content_prefers_native_paths() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("clip.mov");
        std::fs::write(&file, b"mov").unwrap();
        let content = warpui_core::clipboard::ClipboardContent {
            plain_text: "not-a-path".into(),
            paths: Some(vec![file.to_string_lossy().into_owned()]),
            html: None,
            images: None,
        };
        let paths = paths_from_clipboard_content(&content);
        assert_eq!(paths, vec![file]);
    }
}
