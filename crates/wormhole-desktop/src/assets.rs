use std::borrow::Cow;

use anyhow::{anyhow, Result};
use warpui::AssetProvider;

pub struct EmptyAssets;

impl AssetProvider for EmptyAssets {
    fn get(&self, path: &str) -> Result<Cow<'_, [u8]>> {
        Err(anyhow!("no bundled asset at {path}"))
    }
}
