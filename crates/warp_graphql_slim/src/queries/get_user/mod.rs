use crate::scalars::Time;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OwnerType {
    #[default]
    User,
    Team,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PrincipalType {
    #[default]
    User,
    ServiceAccount,
}

#[derive(Clone, Debug, Default)]
pub struct FirebaseProfile {
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub needs_sso_link: bool,
    pub photo_url: Option<String>,
    pub uid: String,
}

#[derive(Clone, Debug, Default)]
pub struct AnonymousUserPersonalObjectLimits {
    pub env_var_limit: i32,
    pub notebook_limit: i32,
    pub workflow_limit: i32,
}

#[derive(Clone, Debug, Default)]
pub struct AnonymousUserInfo {
    pub anonymous_user_type: crate::mutations::create_anonymous_user::AnonymousUserType,
    pub linked_at: Option<Time>,
    pub personal_object_limits: Option<AnonymousUserPersonalObjectLimits>,
}

#[derive(Clone, Debug, Default)]
pub struct FeatureModelChoice;

#[derive(Clone, Debug, Default)]
pub struct User {
    pub anonymous_user_info: Option<AnonymousUserInfo>,
    pub experiments: Option<Vec<crate::experiment::Experiment>>,
    pub global_skills: Vec<String>,
    pub is_onboarded: bool,
    pub is_on_work_domain: bool,
    pub profile: FirebaseProfile,
    pub llms: FeatureModelChoice,
}

#[derive(Clone, Debug, Default)]
pub struct UserOutput {
    pub api_key_owner_type: Option<OwnerType>,
    pub principal_type: Option<PrincipalType>,
    pub user: User,
}
