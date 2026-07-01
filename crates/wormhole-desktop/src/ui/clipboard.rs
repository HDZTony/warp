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
