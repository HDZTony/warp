use pathfinder_geometry::rect::RectF;
use pathfinder_geometry::vector::Vector2F;

use crate::elements::Point;
use crate::event::DispatchedEvent;
use crate::ui_automation::AutomationHit;
use crate::{
    AfterLayoutContext, AppContext, Element, EventContext, LayoutContext, PaintContext,
    SizeConstraint,
};

/// Marks a subtree as an Agent UI-automation target (outline + hit-click).
///
/// Use when the interactive surface is not a [`super::Hoverable`]
/// (e.g. [`super::EventHandler`] buttons).
pub struct AutomationTarget {
    child: Box<dyn Element>,
    label: Option<String>,
    stable_id: Option<String>,
}

impl AutomationTarget {
    pub fn new(child: Box<dyn Element>) -> Self {
        Self {
            child,
            label: None,
            stable_id: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.stable_id = Some(id.into());
        self
    }
}

impl Element for AutomationTarget {
    fn layout(
        &mut self,
        constraint: SizeConstraint,
        ctx: &mut LayoutContext,
        app: &AppContext,
    ) -> Vector2F {
        self.child.layout(constraint, ctx, app)
    }

    fn after_layout(&mut self, ctx: &mut AfterLayoutContext, app: &AppContext) {
        self.child.after_layout(ctx, app);
    }

    fn paint(&mut self, origin: Vector2F, ctx: &mut PaintContext, app: &AppContext) {
        if let Some(size) = self.child.size() {
            let view_id = ctx
                .painting_view_id
                .unwrap_or_else(|| crate::EntityId::from_usize(0));
            ctx.automation_cache.register(AutomationHit {
                label: self.label.clone(),
                stable_id: self.stable_id.clone(),
                bounds: RectF::new(origin, size),
                view_id,
            });
        }
        self.child.paint(origin, ctx, app);
    }

    fn dispatch_event(
        &mut self,
        event: &DispatchedEvent,
        ctx: &mut EventContext,
        app: &AppContext,
    ) -> bool {
        self.child.dispatch_event(event, ctx, app)
    }

    fn size(&self) -> Option<Vector2F> {
        self.child.size()
    }

    fn origin(&self) -> Option<Point> {
        self.child.origin()
    }
}
