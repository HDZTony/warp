//! GQL → app auth model conversions for `wormhole-slim` (avoids orphan-rule impls).

use crate::auth::user::{
    AnonymousUserType, PersonalObjectLimits, PrincipalType, UserMetadata,
};

pub fn firebase_profile_to_metadata(
    profile: warp_graphql::queries::get_user::FirebaseProfile,
) -> UserMetadata {
    UserMetadata {
        email: profile.email.unwrap_or_default(),
        display_name: profile.display_name,
        photo_url: profile.photo_url,
    }
}

pub fn gql_principal_type_to_auth(
    value: warp_graphql::queries::get_user::PrincipalType,
) -> PrincipalType {
    match value {
        warp_graphql::queries::get_user::PrincipalType::User => PrincipalType::User,
        warp_graphql::queries::get_user::PrincipalType::ServiceAccount => {
            PrincipalType::ServiceAccount
        }
    }
}

pub fn gql_anonymous_user_type_to_auth(
    value: warp_graphql::mutations::create_anonymous_user::AnonymousUserType,
) -> Option<AnonymousUserType> {
    match value {
        warp_graphql::mutations::create_anonymous_user::AnonymousUserType::NativeClientAnonymousUser => {
            Some(AnonymousUserType::NativeClientAnonymousUser)
        }
        warp_graphql::mutations::create_anonymous_user::AnonymousUserType::NativeClientAnonymousUserFeatureGated => {
            Some(AnonymousUserType::NativeClientAnonymousUserFeatureGated)
        }
        warp_graphql::mutations::create_anonymous_user::AnonymousUserType::Unknown => None,
    }
}

pub fn gql_personal_object_limits_to_auth(
    limits: warp_graphql::queries::get_user::AnonymousUserPersonalObjectLimits,
) -> PersonalObjectLimits {
    PersonalObjectLimits {
        env_var_limit: limits.env_var_limit.max(0) as usize,
        notebook_limit: limits.notebook_limit.max(0) as usize,
        workflow_limit: limits.workflow_limit.max(0) as usize,
    }
}

pub fn gql_feature_models_to_llms(
    _value: warp_graphql::queries::get_user::FeatureModelChoice,
) -> crate::ai::llms::ModelsByFeature {
    crate::ai::llms::ModelsByFeature::default()
}
