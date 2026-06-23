#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnonymousUserType {
    #[default]
    Unknown,
    NativeClientAnonymousUser,
    NativeClientAnonymousUserFeatureGated,
}

#[derive(Clone, Debug)]
pub enum CreateAnonymousUserResult {
    Success,
    Unknown,
}
