//! In-process UI automation for Agent observe→act loops (desktop sim-use).
//!
//! Interactive [`Hoverable`](crate::elements::Hoverable) targets register into an
//! [`AutomationCache`] during paint. Agents read a flat outline (`@N` aliases) and
//! synthesize real mouse hit-tests at bounds centers — not VoiceOver
//! [`AccessibilityContent`](crate::accessibility::AccessibilityContent) alone
//! (that API has no tree, no bounds, and is a no-op on winit/Windows).

use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;
use serde::{Deserialize, Serialize};

use crate::EntityId;

/// One interactive hit target discovered during the last paint of a window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiAutomationNode {
    /// Session alias assigned when building an outline (`@1`, `@2`, …).
    pub alias: u32,
    /// Human-readable label (`with_automation_label` or `?` when missing).
    pub label: String,
    /// Optional stable id for `#id` selectors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_id: Option<String>,
    /// Window-local bounds (pre-zoom coordinates matching event positions).
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// View that owned the element when painted.
    pub view_id: usize,
}

impl UiAutomationNode {
    pub fn bounds(&self) -> RectF {
        RectF::new(
            Vector2F::new(self.x, self.y),
            Vector2F::new(self.width, self.height),
        )
    }

    pub fn center(&self) -> Vector2F {
        self.bounds().center()
    }
}

/// Raw registration before `@N` assignment (one frame).
#[derive(Debug, Clone)]
pub struct AutomationHit {
    pub label: Option<String>,
    pub stable_id: Option<String>,
    pub bounds: RectF,
    pub view_id: EntityId,
}

/// Cleared at the start of each `build_scene` paint pass; filled by interactive
/// elements during paint.
#[derive(Debug, Default, Clone)]
pub struct AutomationCache {
    hits: Vec<AutomationHit>,
}

impl AutomationCache {
    pub fn clear(&mut self) {
        self.hits.clear();
    }

    pub fn register(&mut self, hit: AutomationHit) {
        if hit.bounds.width() <= 0.0 || hit.bounds.height() <= 0.0 {
            return;
        }
        self.hits.push(hit);
    }

    pub fn hits(&self) -> &[AutomationHit] {
        &self.hits
    }

    /// Flatten hits into numbered nodes. Larger (likely outer) targets first so
    /// nested chrome stays discoverable; agents still pick by label/`@N`.
    pub fn to_nodes(&self) -> Vec<UiAutomationNode> {
        let mut indexed: Vec<(usize, &AutomationHit)> = self.hits.iter().enumerate().collect();
        indexed.sort_by(|(_, a), (_, b)| {
            let area_a = a.bounds.width() * a.bounds.height();
            let area_b = b.bounds.width() * b.bounds.height();
            area_b
                .partial_cmp(&area_a)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.bounds
                        .origin_y()
                        .partial_cmp(&b.bounds.origin_y())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    a.bounds
                        .origin_x()
                        .partial_cmp(&b.bounds.origin_x())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        indexed
            .into_iter()
            .enumerate()
            .map(|(i, (_, hit))| {
                let label = hit
                    .label
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("?")
                    .to_string();
                UiAutomationNode {
                    alias: (i + 1) as u32,
                    label,
                    stable_id: hit.stable_id.clone(),
                    x: hit.bounds.origin_x(),
                    y: hit.bounds.origin_y(),
                    width: hit.bounds.width(),
                    height: hit.bounds.height(),
                    view_id: hit.view_id.to_usize(),
                }
            })
            .collect()
    }
}

/// Format a compact text outline for agents (prefer over JSON).
pub fn format_outline(nodes: &[UiAutomationNode]) -> String {
    if nodes.is_empty() {
        return "(no interactive targets — paint a frame first; add with_automation_label on Hoverables)".to_string();
    }
    let mut lines = Vec::with_capacity(nodes.len());
    for n in nodes {
        let id = n
            .stable_id
            .as_deref()
            .map(|s| format!(" #{s}"))
            .unwrap_or_default();
        lines.push(format!(
            "@{} [{}]{} ({:.0},{:.0} {:.0}x{:.0})",
            n.alias, n.label, id, n.x, n.y, n.width, n.height
        ));
    }
    lines.join("\n")
}

/// Resolve a tap selector against the last outline cache.
#[derive(Debug, Clone)]
pub enum UiTapSelector {
    Alias(u32),
    StableId(String),
    Label {
        label: String,
        role_hint: Option<String>,
    },
}

impl UiTapSelector {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err("empty tap selector".into());
        }
        if let Some(rest) = raw.strip_prefix('@') {
            let n: u32 = rest
                .parse()
                .map_err(|_| format!("invalid alias selector: {raw}"))?;
            if n == 0 {
                return Err("@0 is invalid; aliases start at @1".into());
            }
            return Ok(Self::Alias(n));
        }
        if let Some(rest) = raw.strip_prefix('#') {
            if rest.is_empty() {
                return Err("empty #id".into());
            }
            return Ok(Self::StableId(rest.to_string()));
        }
        Ok(Self::Label {
            label: raw.to_string(),
            role_hint: None,
        })
    }

    pub fn resolve<'a>(&self, nodes: &'a [UiAutomationNode]) -> Result<&'a UiAutomationNode, String> {
        match self {
            Self::Alias(n) => nodes
                .iter()
                .find(|node| node.alias == *n)
                .ok_or_else(|| {
                    format!(
                        "stale or unknown alias @{n}; re-run ui_outline ({} targets cached)",
                        nodes.len()
                    )
                }),
            Self::StableId(id) => {
                let matches: Vec<_> = nodes
                    .iter()
                    .filter(|n| n.stable_id.as_deref() == Some(id.as_str()))
                    .collect();
                match matches.as_slice() {
                    [one] => Ok(*one),
                    [] => Err(format!("no target with id #{id}")),
                    _ => Err(format!("ambiguous id #{id}: {} matches", matches.len())),
                }
            }
            Self::Label { label, .. } => {
                let needle = label.trim();
                let exact: Vec<_> = nodes
                    .iter()
                    .filter(|n| n.label == needle)
                    .collect();
                if exact.len() == 1 {
                    return Ok(exact[0]);
                }
                let ci: Vec<_> = nodes
                    .iter()
                    .filter(|n| n.label.eq_ignore_ascii_case(needle))
                    .collect();
                if ci.len() == 1 {
                    return Ok(ci[0]);
                }
                let contains: Vec<_> = nodes
                    .iter()
                    .filter(|n| n.label.contains(needle))
                    .collect();
                match contains.as_slice() {
                    [one] => Ok(*one),
                    [] => Err(format!("no target with label '{needle}'")),
                    many => Err(format!(
                        "ambiguous label '{needle}': {} matches ({})",
                        many.len(),
                        many.iter()
                            .take(5)
                            .map(|n| format!("@{} [{}]", n.alias, n.label))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pathfinder_geometry::vector::vec2f;

    fn hit(label: &str, x: f32, y: f32, w: f32, h: f32) -> AutomationHit {
        AutomationHit {
            label: Some(label.into()),
            stable_id: None,
            bounds: RectF::new(vec2f(x, y), vec2f(w, h)),
            view_id: EntityId::from_usize(1),
        }
    }

    #[test]
    fn assigns_aliases_and_resolves_label() {
        let mut cache = AutomationCache::default();
        cache.register(hit("设置", 10.0, 0.0, 80.0, 40.0));
        cache.register(hit("聊天", 90.0, 0.0, 80.0, 40.0));
        let nodes = cache.to_nodes();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].alias, 1);
        let sel = UiTapSelector::parse("设置").unwrap();
        assert_eq!(sel.resolve(&nodes).unwrap().label, "设置");
        let alias = UiTapSelector::parse("@1").unwrap();
        assert!(alias.resolve(&nodes).is_ok());
    }

    #[test]
    fn stale_alias_errors() {
        let nodes = AutomationCache::default().to_nodes();
        let err = UiTapSelector::Alias(3).resolve(&nodes).unwrap_err();
        assert!(err.contains("stale") || err.contains("unknown"));
    }

    #[test]
    fn format_outline_empty() {
        let text = format_outline(&[]);
        assert!(text.contains("no interactive"));
    }
}
