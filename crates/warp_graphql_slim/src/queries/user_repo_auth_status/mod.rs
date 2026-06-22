use crate::client::Id;
use crate::scalars::Time;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UserRepoAuthStatusEnum {
    #[default]
    Unknown,
    NoInstallationOrAccessForRepo,
    UserNotConnectedToGithub,
    Success,
}

#[derive(Clone, Debug, Default)]
pub struct RepoResult {
    pub owner: String,
    pub repo: String,
    pub status: UserRepoAuthStatusEnum,
    pub is_public: bool,
}

#[derive(Clone, Debug, Default)]
pub struct UserRepoAuthStatusOutput {
    pub statuses: Vec<RepoResult>,
    pub auth_url: Option<String>,
    pub tx_id: Option<Id>,
}
