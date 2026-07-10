//! Windows PE resources for `wormhole-desktop`.
//!
//! WarpUI loads the taskbar / window icon via `winit::window::Icon::from_resource(0x101)`
//! (`warpui::windowing::winit::window::IDI_ICON`). That ID must match the embedded
//! `RT_GROUP_ICON` — the same convention as Warp's `app/build.rs` (`#define IDI_ICON 0x101`).
//!
//! `winres::WindowsResource::set_icon` embeds as ID `1`, which WarpUI never looks up, so the
//! taskbar stays a blank/generic glyph even though Explorer can still extract an icon from the
//! exe. Always use ID `257` (`0x101`) here.

fn main() {
    #[cfg(target_os = "windows")]
    {
        /// Must match `IDI_ICON` in `warpui` (`0x101`) and Warp `app/build.rs`.
        const WARPUI_IDI_ICON: &str = "257";

        let icon = "../../../../apps/desktop/bundle/icons/icon.ico";
        println!("cargo:rerun-if-changed={icon}");
        if !std::path::Path::new(icon).is_file() {
            println!(
                "cargo:warning=missing Windows icon at {icon}; run: uv run apps/desktop/scripts/sync_app_icons.py --windows-only"
            );
            return;
        }

        let mut resource = winres::WindowsResource::new();
        resource.set_icon_with_id(icon, WARPUI_IDI_ICON);
        if let Err(error) = resource.compile() {
            println!("cargo:warning=failed to compile Windows icon resources: {error}");
        }
    }
}
