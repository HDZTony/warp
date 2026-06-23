use crate::client::Id;
use crate::error::UserFacingErrorInterface;
use crate::scalars::Time;

#[derive(Clone, Debug, Default)]
pub struct GithubConnectedOutput {
    pub username: Option<String>,
    pub installed_repos: Vec<RepoResult>,
    pub app_install_link: String,
}

#[derive(Clone, Debug, Default)]
pub struct GithubAuthRequiredOutput {
    pub auth_url: String,
    pub tx_id: Id,
    pub app_install_link: String,
}

#[derive(Clone, Debug, Default)]
pub struct RepoResult {
    pub owner: String,
    pub repo: String,
    pub is_public: bool,
}

#[derive(Clone, Debug)]
pub enum UserGithubInfoResult {
    GithubConnectedOutput(GithubConnectedOutput),
    GithubAuthRequiredOutput(GithubAuthRequiredOutput),
    Unknown,
}
