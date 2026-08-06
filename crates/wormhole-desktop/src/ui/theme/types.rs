//! Theme data model (OpenHuman Theme Studio parity, Wormhole shell tokens).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Light / Dark / follow OS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeVariant {
    Light,
    #[default]
    Dark,
    System,
}

impl ThemeVariant {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            "system" | "auto" => Some(Self::System),
            _ => None,
        }
    }
}

/// Backdrop layer behind HUD content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackdropKind {
    #[default]
    Mesh,
    Solid,
    Image,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeBackdrop {
    pub kind: BackdropKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
    /// Show dotted / grid overlay when kind is Mesh (default true).
    #[serde(default = "default_true")]
    pub dots: bool,
}

fn default_true() -> bool {
    true
}

impl Default for ThemeBackdrop {
    fn default() -> Self {
        Self {
            kind: BackdropKind::Mesh,
            image_url: None,
            dots: true,
        }
    }
}

/// Font role keys stored on a theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontRole {
    Title,
    Heading,
    Body,
    Mono,
    Serif,
}

impl FontRole {
    pub const ALL: [FontRole; 5] = [
        Self::Title,
        Self::Heading,
        Self::Body,
        Self::Mono,
        Self::Serif,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Heading => "heading",
            Self::Body => "body",
            Self::Mono => "mono",
            Self::Serif => "serif",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "title" => Some(Self::Title),
            "heading" => Some(Self::Heading),
            "body" => Some(Self::Body),
            "mono" => Some(Self::Mono),
            "serif" => Some(Self::Serif),
            _ => None,
        }
    }
}

/// A concrete theme: partial overrides over Classic light/dark baselines.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub is_dark: bool,
    pub built_in: bool,
    /// Preset variant id this custom theme was forked from (e.g. `ocean-dark`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    /// Token key → `#RRGGBB` hex.
    #[serde(default)]
    pub colors: HashMap<String, String>,
    /// Font role → font choice id (see [`super::tokens::FONT_CHOICES`]).
    #[serde(default)]
    pub fonts: HashMap<String, String>,
    #[serde(default)]
    pub backdrop: ThemeBackdrop,
}

/// Family grouping light + dark variants under one gallery tile.
#[derive(Clone, Copy, Debug)]
pub struct ThemeFamily {
    pub id: &'static str,
    pub name: &'static str,
    pub default_variant: ThemeVariant,
    pub light_id: Option<&'static str>,
    pub dark_id: Option<&'static str>,
}
