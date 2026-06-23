fn main() {
    #[cfg(target_os = "windows")]
    {
        let icon = "../../../../apps/desktop/bundle/icons/icon.ico";
        if !std::path::Path::new(icon).is_file() {
            println!(
                "cargo:warning=missing Windows icon at {icon}; run: uv run apps/desktop/scripts/sync_app_icons.py --windows-only"
            );
            return;
        }

        let mut resource = winres::WindowsResource::new();
        resource.set_icon(icon);
        if let Err(error) = resource.compile() {
            println!("cargo:warning=failed to compile Windows icon resources: {error}");
        }
    }
}
