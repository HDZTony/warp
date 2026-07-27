//! Decode local image files into Warp `AssetCache` payloads for chat wallpapers
//! and image attachments.

use std::io::Read;
use std::path::Path;

use warpui::assets::asset_cache::AssetCache;
use warpui::{SingletonEntity, ViewContext};
use warpui_core::image_cache::{CustomImageFormat, CustomImageHeader, ImageType};

pub fn chat_wallpaper_asset_id(conv_id: &str) -> String {
    format!("wormhole-chat-wallpaper-{conv_id}")
}

pub fn chat_attachment_asset_id(attachment_id: &str) -> String {
    format!("wormhole-chat-attachment-{attachment_id}")
}

pub fn decode_image_asset_payload(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    let image = reader.decode().map_err(|err| err.to_string())?.to_rgb8();
    let (width, height) = image.dimensions();
    CustomImageHeader::prepend_custom_header(
        image.into_raw(),
        width,
        height,
        CustomImageFormat::Rgb,
    )
    .map_err(|err| format!("{err:?}"))
}

pub fn load_wallpaper_bytes_from_path(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = std::fs::File::open(path).map_err(|err| format!("无法读取壁纸: {err}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|err| format!("无法读取壁纸: {err}"))?;
    Ok(bytes)
}

pub fn load_image_bytes_from_path(path: &Path) -> Result<Vec<u8>, String> {
    let mut file = std::fs::File::open(path).map_err(|err| format!("无法读取图片: {err}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|err| format!("无法读取图片: {err}"))?;
    Ok(bytes)
}

pub fn insert_wallpaper_asset<V: warpui::View>(
    ctx: &mut ViewContext<V>,
    conv_id: &str,
    bytes: Vec<u8>,
) -> Result<(), String> {
    let payload = decode_image_asset_payload(bytes)?;
    let asset_id = chat_wallpaper_asset_id(conv_id);
    AssetCache::handle(ctx).update(ctx, |cache, model_ctx| {
        cache.insert_raw_asset_bytes::<ImageType>(asset_id, &payload, model_ctx);
    });
    Ok(())
}

/// Insert a chat image attachment into the asset cache. Returns the asset id on success.
pub fn insert_attachment_image_asset<V: warpui::View>(
    ctx: &mut ViewContext<V>,
    attachment_id: &str,
    path: &Path,
) -> Result<String, String> {
    let bytes = load_image_bytes_from_path(path)?;
    let payload = decode_image_asset_payload(bytes)?;
    let asset_id = chat_attachment_asset_id(attachment_id);
    AssetCache::handle(ctx).update(ctx, |cache, model_ctx| {
        cache.insert_raw_asset_bytes::<ImageType>(asset_id.clone(), &payload, model_ctx);
    });
    Ok(asset_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallpaper_asset_id_is_stable_per_conv() {
        assert_eq!(
            chat_wallpaper_asset_id("conv-a"),
            "wormhole-chat-wallpaper-conv-a"
        );
    }

    #[test]
    fn attachment_asset_id_is_stable() {
        assert_eq!(
            chat_attachment_asset_id("att-1"),
            "wormhole-chat-attachment-att-1"
        );
    }
}
