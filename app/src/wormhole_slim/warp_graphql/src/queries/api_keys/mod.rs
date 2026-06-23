use crate::object_permissions::OwnerType;
use crate::scalars::Time;

#[derive(Clone, Debug, Default)]
pub struct ApiKeyAgentInfo {
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct ApiKeyUid(pub String);

impl ApiKeyUid {
    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Clone, Debug, Default)]
pub struct ApiKeyProperties {
    pub uid: ApiKeyUid,
    pub name: String,
    pub key_suffix: String,
    pub owner_type: OwnerType,
    pub agent_info: Option<ApiKeyAgentInfo>,
    pub created_at: Time,
    pub last_used_at: Option<Time>,
    pub expires_at: Option<Time>,
}
