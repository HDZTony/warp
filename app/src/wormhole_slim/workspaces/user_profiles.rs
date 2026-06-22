//! Minimal [`UserProfiles`] stub for `wormhole-slim` embed builds.

pub use cloud_object_models::UserProfileWithUID;
use warpui::{Entity, SingletonEntity};

use crate::auth::UserUid;

pub enum UserProfilesEvent {}

#[cfg(not(target_family = "wasm"))]
pub fn user_profile_from_persistence(
    user_profile: crate::persistence::model::UserProfile,
) -> UserProfileWithUID {
    UserProfileWithUID {
        firebase_uid: UserUid::new(&user_profile.firebase_uid),
        display_name: user_profile.display_name,
        email: user_profile.email,
        photo_url: user_profile.photo_url,
    }
}

pub struct UserProfileData {
    pub display_name: Option<String>,
    pub email: String,
    #[allow(dead_code)]
    pub photo_url: String,
}

impl UserProfileData {
    pub fn displayable_identifier(&self) -> String {
        self.display_name
            .clone()
            .unwrap_or_else(|| self.email.clone())
    }
}

pub struct UserProfiles {}

impl UserProfiles {
    pub fn new(_user_profiles: Vec<UserProfileWithUID>) -> Self {
        Self {}
    }

    pub fn insert_profiles(&mut self, _user_profiles: &Vec<UserProfileWithUID>) {}

    pub fn clear_profiles(&mut self) {}

    pub fn profile_for_uid(&self, _uid: UserUid) -> Option<&UserProfileData> {
        None
    }

    pub fn displayable_identifier_for_uid(&self, _uid: UserUid) -> Option<String> {
        None
    }

    pub fn displayable_identifier_for_email(&self, _email: &str) -> Option<String> {
        None
    }
}

impl Entity for UserProfiles {
    type Event = UserProfilesEvent;
}

impl SingletonEntity for UserProfiles {}
