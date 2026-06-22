#[derive(Clone, Debug)]
pub enum UntrashObjectResult {
    Unknown,
    UserFacingError(crate::error::UserFacingErrorInterface),
}

