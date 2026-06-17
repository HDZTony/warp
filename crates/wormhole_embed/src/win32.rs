use std::ffi::c_void;

use anyhow::{Context, Result};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetParent, GetWindowLongPtrW, GetWindowThreadProcessId, IsWindowVisible,
    SetParent, SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_STYLE, HWND_BOTTOM, SWP_NOACTIVATE,
    SWP_SHOWWINDOW, SW_SHOW, WS_CHILD, WS_POPUP,
};

struct SearchState {
    pid: u32,
    hwnd: Option<HWND>,
}

pub fn find_visible_window_for_pid(pid: u32) -> Option<HWND> {
    let mut search = SearchState { pid, hwnd: None };
    unsafe {
        let _ = EnumWindows(Some(enum_for_pid), LPARAM(&mut search as *mut _ as isize));
    }
    search.hwnd
}

unsafe extern "system" fn enum_for_pid(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let search = &mut *(lparam.0 as *mut SearchState);
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let mut window_pid = 0u32;
    let _ = GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
    if window_pid == search.pid {
        let parent = GetParent(hwnd).unwrap_or(HWND::default());
        if parent.0.is_null() {
            search.hwnd = Some(hwnd);
            return BOOL(0);
        }
    }
    BOOL(1)
}

pub fn embed_window_into_host(child: HWND, host: HWND) -> Result<()> {
    if child.0.is_null() || host.0.is_null() {
        anyhow::bail!("invalid hwnd for embed");
    }
    unsafe {
        let style = GetWindowLongPtrW(child, GWL_STYLE);
        let style = (style as u32) & !WS_POPUP.0;
        let style = style | WS_CHILD.0;
        SetWindowLongPtrW(child, GWL_STYLE, style as _);
        SetParent(child, Some(host)).context("SetParent failed")?;
        let _ = ShowWindow(child, SW_SHOW);
    }
    Ok(())
}

pub fn resize_embedded(child: HWND, width: i32, height: i32) {
    if child.0.is_null() {
        return;
    }
    unsafe {
        let _ = SetWindowPos(
            child,
            Some(HWND_BOTTOM),
            0,
            0,
            width.max(1),
            height.max(1),
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

pub fn hwnd_from_isize(value: isize) -> HWND {
    HWND(value as *mut c_void)
}
