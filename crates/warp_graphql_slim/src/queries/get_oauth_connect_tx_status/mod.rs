#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OauthConnectTxStatus {
    Completed,
    Failed,
    Expired,
    Pending,
    InProgress,
    Unknown,
}

impl Default for OauthConnectTxStatus {
    fn default() -> Self {
        Self::Unknown
    }
}
