//! [`UserWorkspaces`] stub for `wormhole-slim` embed builds.

use std::sync::Arc;

use warpui::{AppContext, Entity, ModelContext, SingletonEntity};

use super::team::{DiscoverableTeam, Team};
use super::workspace::{
    AdminEnablementSetting, AiAutonomySettings, EnterpriseSecretRegex, HostEnablementSetting,
    SandboxedAgentSettings, UgcCollectionEnablementSetting, UsageBasedPricingSettings, Workspace,
    WorkspaceUid,
};
use crate::cloud_object::CloudObjectEventEntrypoint;
use crate::auth::UserUid;
use crate::cloud_object::{Owner, Space};
use crate::server::ids::ServerId;
use crate::server::server_api::team::TeamClient;
use crate::server::server_api::workspace::WorkspaceClient;

#[derive(Debug)]
pub enum UserWorkspacesEvent {
    AddDomainRestrictionsSuccess,
    AddDomainRestrictionsRejected(anyhow::Error),
    DeleteDomainRestrictionSuccess,
    DeleteDomainRestrictionRejected(anyhow::Error),
    EmailInviteSent,
    EmailInviteRejected(anyhow::Error),
    ToggleInviteLinksSuccess,
    ToggleInviteLinksRejected(anyhow::Error),
    ResetInviteLinks,
    ResetInviteLinksRejected(anyhow::Error),
    DeleteTeamInvite,
    DeleteTeamInviteRejected(anyhow::Error),
    GenerateUpgradeLink(String),
    GenerateUpgradeLinkRejected(anyhow::Error),
    GenerateStripeBillingPortalLink(String),
    GenerateStripeBillingPortalLinkRejected(anyhow::Error),
    ToggleTeamDiscoverabilitySuccess,
    ToggleTeamDiscoverabilityRejected(anyhow::Error),
    JoinTeamWithTeamDiscoverySuccess,
    JoinTeamWithTeamDiscoveryRejected(anyhow::Error),
    FetchDiscoverableTeamsSuccess(Vec<DiscoverableTeam>),
    FetchDiscoverableTeamsRejected(anyhow::Error),
    TransferTeamOwnershipSuccess,
    TransferTeamOwnershipRejected(anyhow::Error),
    SetTeamMemberRoleSuccess,
    SetTeamMemberRoleRejected(anyhow::Error),
    UpdateWorkspaceSettingsSuccess,
    UpdateWorkspaceSettingsRejected(anyhow::Error),
    AiOveragesUpdated,
    PurchaseAddonCreditsSuccess,
    PurchaseAddonCreditsRejected(anyhow::Error),
    TeamsChanged,
    CodebaseContextEnablementChanged,
    SunsettedToBuildDataUpdated,
}

pub struct UserWorkspaces {
    current_workspace_uid: Option<WorkspaceUid>,
    workspaces: Vec<Workspace>,
    joinable_teams: Vec<DiscoverableTeam>,
    #[allow(dead_code)]
    team_client: Arc<dyn TeamClient>,
    #[allow(dead_code)]
    workspace_client: Arc<dyn WorkspaceClient>,
}

impl UserWorkspaces {
    pub fn new(
        team_client: Arc<dyn TeamClient>,
        workspace_client: Arc<dyn WorkspaceClient>,
        cached_workspaces: Vec<Workspace>,
        current_workspace_uid: Option<WorkspaceUid>,
        _ctx: &mut ModelContext<Self>,
    ) -> Self {
        Self {
            current_workspace_uid,
            workspaces: cached_workspaces,
            joinable_teams: Vec::new(),
            team_client,
            workspace_client,
        }
    }

    pub fn upgrade_link(_user_id: UserUid) -> String {
        String::new()
    }

    pub fn upgrade_link_for_team(_team_uid: ServerId) -> String {
        String::new()
    }

    pub fn team_from_uid(&self, _team_uid: ServerId) -> Option<&Team> {
        None
    }

    pub fn current_team_uid(&self) -> Option<ServerId> {
        None
    }

    pub fn current_team(&self) -> Option<&Team> {
        None
    }

    pub fn current_team_mut(&mut self) -> Option<&mut Team> {
        None
    }

    pub fn current_workspace(&self) -> Option<&Workspace> {
        self.current_workspace_uid
            .as_ref()
            .and_then(|uid| self.workspaces.iter().find(|w| w.uid == *uid))
    }

    pub fn current_workspace_mut(&mut self) -> Option<&mut Workspace> {
        let uid = self.current_workspace_uid.as_ref()?;
        self.workspaces.iter_mut().find(|w| w.uid == *uid)
    }

    pub fn workspaces(&self) -> &Vec<Workspace> {
        &self.workspaces
    }

    pub fn personal_drive(&self, _ctx: &AppContext) -> Option<Owner> {
        None
    }

    pub fn space_to_owner(&self, _space: Space, _ctx: &AppContext) -> Option<Owner> {
        None
    }

    pub fn owner_to_space(&self, owner: Owner, _ctx: &AppContext) -> Space {
        match owner {
            Owner::User { .. } => Space::Personal,
            Owner::Team { team_uid } => Space::Team { team_uid },
        }
    }

    pub fn has_capacity_for_shared_workflows(
        _team_uid: ServerId,
        _ctx: &AppContext,
        _count: usize,
    ) -> bool {
        true
    }

    pub fn is_at_tier_limit_for_object_type(
        _team_uid: ServerId,
        _object_type: cloud_objects::cloud_object::ObjectType,
        _ctx: &AppContext,
    ) -> bool {
        false
    }

    pub fn ai_allowed_for_current_team(&self) -> bool {
        true
    }

    pub fn get_remote_session_regex_list(&self) -> Vec<regex::Regex> {
        Vec::new()
    }

    pub fn is_byo_api_key_enabled(&self, _app: &AppContext) -> bool {
        false
    }

    pub fn is_codebase_context_enabled(&self, _app: &AppContext) -> bool {
        false
    }

    pub fn is_voice_enabled(&self) -> bool {
        false
    }

    pub fn ai_autonomy_settings(&self) -> AiAutonomySettings {
        AiAutonomySettings::default()
    }

    pub fn usage_based_pricing_settings(&self) -> UsageBasedPricingSettings {
        UsageBasedPricingSettings::default()
    }

    pub fn get_cloud_conversation_storage_enablement_setting(&self) -> AdminEnablementSetting {
        AdminEnablementSetting::default()
    }

    pub fn get_ugc_collection_enablement_setting(&self) -> UgcCollectionEnablementSetting {
        UgcCollectionEnablementSetting::default()
    }

    pub fn get_enterprise_secret_redaction_regex_list(&self) -> Vec<EnterpriseSecretRegex> {
        Vec::new()
    }

    pub fn aws_bedrock_host_enablement_setting(&self) -> HostEnablementSetting {
        HostEnablementSetting::default()
    }

    pub fn set_current_workspace_uid(
        &mut self,
        workspace_uid: Option<WorkspaceUid>,
        _ctx: &mut ModelContext<Self>,
    ) {
        self.current_workspace_uid = workspace_uid;
    }

    pub fn update_workspaces(
        &mut self,
        workspaces: Vec<Workspace>,
        _ctx: &mut ModelContext<Self>,
    ) {
        self.workspaces = workspaces;
    }

    pub fn has_capacity_for_shared_notebooks(
        _team_uid: ServerId,
        _ctx: &AppContext,
        _new_shared_notebooks: usize,
    ) -> bool {
        true
    }

    pub fn is_aws_bedrock_credentials_enabled(&self, _app: &AppContext) -> bool {
        false
    }

    pub fn is_aws_bedrock_available_from_workspace(&self) -> bool {
        false
    }

    pub fn is_aws_bedrock_credentials_toggleable(&self) -> bool {
        false
    }

    pub fn is_custom_inference_enabled(&self, _app: &AppContext) -> bool {
        false
    }

    pub fn is_code_suggestions_toggleable(&self) -> bool {
        false
    }

    pub fn is_prompt_suggestions_toggleable(&self) -> bool {
        false
    }

    pub fn is_next_command_enabled(&self) -> bool {
        false
    }

    pub fn is_git_operations_ai_enabled(&self) -> bool {
        false
    }

    pub fn is_ai_allowed_in_remote_sessions(&self) -> bool {
        false
    }

    pub fn is_ai_autonomy_allowed(&self) -> bool {
        false
    }

    pub fn is_enterprise_secret_redaction_enabled(&self) -> bool {
        false
    }

    pub fn has_teams(&self) -> bool {
        false
    }

    pub fn num_joinable_teams(&self) -> usize {
        0
    }

    pub fn total_teammates_in_joinable_teams(&self) -> i64 {
        0
    }

    pub fn default_host_slug(&self) -> Option<&str> {
        None
    }

    pub fn all_user_spaces(&self, _ctx: &AppContext) -> Vec<Space> {
        Vec::new()
    }

    pub fn team_spaces(&self) -> Vec<Space> {
        Vec::new()
    }

    pub fn team_from_uid_across_all_workspaces(&self, _team_uid: ServerId) -> Option<&Team> {
        None
    }

    pub fn team_allows_codebase_context(&self) -> AdminEnablementSetting {
        AdminEnablementSetting::default()
    }

    pub fn is_anyone_with_link_sharing_enabled(&self) -> bool {
        false
    }

    pub fn is_direct_link_sharing_enabled(&self) -> bool {
        false
    }

    pub fn get_agent_attribution_setting(&self) -> AdminEnablementSetting {
        AdminEnablementSetting::default()
    }

    pub fn sandboxed_agent_settings(&self) -> Option<SandboxedAgentSettings> {
        None
    }

    pub fn refresh_ai_overages(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn update_addon_credits_settings(
        &mut self,
        _team_uid: ServerId,
        _auto_reload_enabled: Option<bool>,
        _max_monthly_spend_cents: Option<i32>,
        _selected_auto_reload_credit_denomination: Option<i32>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn update_usage_based_pricing_settings(
        &mut self,
        _team_uid: ServerId,
        _usage_based_pricing_enabled: bool,
        _max_monthly_spend_cents: Option<u32>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn purchase_addon_credits(
        &mut self,
        _team_uid: ServerId,
        _credits: i32,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn generate_stripe_billing_portal_link(
        &mut self,
        _team_uid: ServerId,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn generate_upgrade_link(
        &mut self,
        _team_uid: ServerId,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn fetch_discoverable_teams(&mut self, _ctx: &mut ModelContext<Self>) {}

    pub fn add_invite_link_domain_restrictions(
        &mut self,
        _team_uid: ServerId,
        _domains: Vec<String>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn delete_invite_link_domain_restriction(
        &mut self,
        _team_uid: ServerId,
        _domain: String,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn send_email_invites(
        &mut self,
        _team_uid: ServerId,
        _emails: Vec<String>,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn set_is_invite_link_enabled(
        &mut self,
        _team_uid: ServerId,
        _enabled: bool,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn reset_invite_links(
        &mut self,
        _team_uid: ServerId,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn set_team_discoverability(
        &mut self,
        _team_uid: ServerId,
        _discoverable: bool,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn join_team_with_team_discovery(
        &mut self,
        _team_uid: ServerId,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn delete_team_invite(
        &mut self,
        _team_uid: ServerId,
        _invite_id: String,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn remove_user_from_team(
        &mut self,
        _user_uid: UserUid,
        _team_uid: ServerId,
        _entrypoint: CloudObjectEventEntrypoint,
        _ctx: &mut ModelContext<Self>,
    ) {
    }

    pub fn set_team_member_role(
        &mut self,
        _user_uid: UserUid,
        _team_uid: ServerId,
        _role: super::team::MembershipRole,
        _ctx: &mut ModelContext<Self>,
    ) {
    }
}

impl Entity for UserWorkspaces {
    type Event = UserWorkspacesEvent;
}

impl SingletonEntity for UserWorkspaces {}

#[cfg(test)]
impl UserWorkspaces {
    pub fn mock(_ctx: &mut ModelContext<Self>) -> Self {
        use crate::server::server_api::{team, workspace};

        Self::new(
            Arc::new(team::StubTeamClient),
            Arc::new(workspace::StubWorkspaceClient),
            Vec::new(),
            None,
            _ctx,
        )
    }
}
