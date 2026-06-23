#[derive(Clone, Debug, Default)]
pub struct WarpDevImage {
    pub image: String,
    pub repository: String,
    pub tag: String,
}

#[derive(Clone, Debug, Default)]
pub struct ListWarpDevImagesOutput {
    pub images: Vec<WarpDevImage>,
}

#[derive(Clone, Debug)]
pub enum ListWarpDevImagesResult {
    ListWarpDevImagesOutput(ListWarpDevImagesOutput),
    UserFacingError(crate::error::UserFacingErrorInterface),
    Unknown,
}

impl Default for ListWarpDevImagesResult {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Clone, Debug, Default)]
pub struct ListWarpDevImagesResponse {
    pub list_warp_dev_images: ListWarpDevImagesResult,
}

#[derive(Clone, Debug, Default)]
pub struct ListWarpDevImages;

#[derive(Clone, Debug, Default)]
pub struct ListWarpDevImagesVariables;

impl ListWarpDevImages {
    pub fn build(
        _vars: ListWarpDevImagesVariables,
    ) -> crate::client::OperationMarker<ListWarpDevImagesResponse> {
        crate::client::OperationMarker::new()
    }
}
