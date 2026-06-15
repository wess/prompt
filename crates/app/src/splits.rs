//! Render the active tab's pane tree: positioned panes, thin draggable
//! dividers, and a border on the focused pane.
//!
//! The element lays its pane children out manually (zed pane-group style):
//! `workspace::compute_layout` turns the tree into rects, each pane entity
//! is prepainted at its rect, and dividers become quads with hitboxes.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, DispatchPhase, Element, ElementId, GlobalElementId,
    Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Style, WeakEntity, Window, fill, point, px, relative,
    size,
};
use workspace::{Axis, Node, PaneId, PaneTree, Rect, SplitId, clamp_ratio, compute_layout};

use crate::root::WorkspaceView;

/// Divider thickness reserved between panes, in pixels.
pub const DIVIDER: f32 = 2.0;

/// Extra grab slack on each side of a divider's hitbox, in pixels.
const GRAB: f32 = 2.0;

/// An in-progress divider drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drag {
    pub split: SplitId,
    pub axis: Axis,
}

/// Rect occupied by the split node `target` when `tree` fills `rect`.
/// Mirrors the rect math of [`workspace::compute_layout`].
pub fn splitrect(tree: &PaneTree, rect: Rect, divider: f32, target: SplitId) -> Option<Rect> {
    noderect(tree.root(), rect, divider, target)
}

fn noderect(node: &Node, rect: Rect, divider: f32, target: SplitId) -> Option<Rect> {
    let Node::Split { id, axis, ratio, first, second } = node else {
        return None;
    };
    if *id == target {
        return Some(rect);
    }
    let (frect, srect) = childrects(rect, *axis, *ratio, divider);
    noderect(first, frect, divider, target).or_else(|| noderect(second, srect, divider, target))
}

fn childrects(rect: Rect, axis: Axis, ratio: f32, divider: f32) -> (Rect, Rect) {
    match axis {
        Axis::Horizontal => {
            let avail = (rect.w - divider).max(0.0);
            let fw = avail * ratio;
            (
                Rect::new(rect.x, rect.y, fw, rect.h),
                Rect::new(rect.x + fw + divider, rect.y, avail - fw, rect.h),
            )
        }
        Axis::Vertical => {
            let avail = (rect.h - divider).max(0.0);
            let fh = avail * ratio;
            (
                Rect::new(rect.x, rect.y, rect.w, fh),
                Rect::new(rect.x, rect.y + fh + divider, rect.w, avail - fh),
            )
        }
    }
}

/// New ratio for dragging the divider of a split occupying `rect` so it
/// lands at the pointer position `pos` (window coordinates).
pub fn dragratio(rect: Rect, axis: Axis, divider: f32, pos: (f32, f32)) -> f32 {
    let (start, span, p) = match axis {
        Axis::Horizontal => (rect.x, (rect.w - divider).max(1.0), pos.0),
        Axis::Vertical => (rect.y, (rect.h - divider).max(1.0), pos.1),
    };
    clamp_ratio((p - start - divider / 2.0) / span)
}

pub struct SplitsElement {
    tree: PaneTree,
    focused: PaneId,
    children: Vec<(PaneId, AnyElement)>,
    dividercolor: Hsla,
    focuscolor: Hsla,
    drag: Rc<RefCell<Option<Drag>>>,
    root: WeakEntity<WorkspaceView>,
}

impl SplitsElement {
    pub fn new(
        tree: PaneTree,
        focused: PaneId,
        children: Vec<(PaneId, AnyElement)>,
        dividercolor: Hsla,
        focuscolor: Hsla,
        drag: Rc<RefCell<Option<Drag>>>,
        root: WeakEntity<WorkspaceView>,
    ) -> Self {
        Self { tree, focused, children, dividercolor, focuscolor, drag, root }
    }
}

pub struct SplitsFrame {
    /// The element bounds as a layout rect, for drag-ratio math.
    rect: Rect,
    panes: Vec<(PaneId, Bounds<Pixels>)>,
    dividers: Vec<(SplitId, Axis, Bounds<Pixels>, Hitbox)>,
}

fn pxbounds(r: Rect) -> Bounds<Pixels> {
    Bounds::new(point(px(r.x), px(r.y)), size(px(r.w), px(r.h)))
}

/// Widen a divider's hitbox across its thin axis for easier grabbing.
fn grabbounds(b: Bounds<Pixels>, axis: Axis) -> Bounds<Pixels> {
    match axis {
        Axis::Horizontal => Bounds::new(
            point(b.origin.x - px(GRAB), b.origin.y),
            size(b.size.width + px(2.0 * GRAB), b.size.height),
        ),
        Axis::Vertical => Bounds::new(
            point(b.origin.x, b.origin.y - px(GRAB)),
            size(b.size.width, b.size.height + px(2.0 * GRAB)),
        ),
    }
}

impl Element for SplitsElement {
    type RequestLayoutState = ();
    type PrepaintState = SplitsFrame;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> SplitsFrame {
        let rect = Rect::new(
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        );
        let layout = compute_layout(&self.tree, rect, DIVIDER);

        let mut panes = Vec::with_capacity(self.children.len());
        for (pane, element) in &mut self.children {
            let Some(r) = layout.pane_rect(*pane) else {
                continue;
            };
            element.layout_as_root(
                size(
                    AvailableSpace::Definite(px(r.w)),
                    AvailableSpace::Definite(px(r.h)),
                ),
                window,
                cx,
            );
            element.prepaint_at(point(px(r.x), px(r.y)), window, cx);
            panes.push((*pane, pxbounds(r)));
        }

        let dividers = layout
            .dividers
            .iter()
            .map(|(split, r, axis)| {
                let b = pxbounds(*r);
                let hitbox = window.insert_hitbox(grabbounds(b, *axis), HitboxBehavior::Normal);
                (*split, *axis, b, hitbox)
            })
            .collect();

        SplitsFrame { rect, panes, dividers }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut (),
        frame: &mut SplitsFrame,
        window: &mut Window,
        cx: &mut App,
    ) {
        for (_, child) in &mut self.children {
            child.paint(window, cx);
        }

        let dragging = *self.drag.borrow();
        for (split, axis, bounds, hitbox) in &frame.dividers {
            window.paint_quad(fill(*bounds, self.dividercolor));
            let cursor = match axis {
                Axis::Horizontal => gpui::CursorStyle::ResizeColumn,
                Axis::Vertical => gpui::CursorStyle::ResizeRow,
            };
            if dragging.is_some_and(|d| d.split == *split) {
                window.set_window_cursor_style(cursor);
            } else {
                window.set_cursor_style(cursor, hitbox);
            }
        }

        if frame.panes.len() > 1 {
            if let Some((_, bounds)) = frame.panes.iter().find(|(p, _)| *p == self.focused) {
                window.paint_quad(gpui::quad(
                    *bounds,
                    0.,
                    gpui::transparent_black(),
                    px(1.0),
                    self.focuscolor,
                    gpui::BorderStyle::Solid,
                ));
            }
        }

        // Divider drag start.
        let drag = self.drag.clone();
        let hitboxes: Vec<(SplitId, Axis, Hitbox)> = frame
            .dividers
            .iter()
            .map(|(split, axis, _, hitbox)| (*split, *axis, hitbox.clone()))
            .collect();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble || event.button != gpui::MouseButton::Left {
                return;
            }
            for (split, axis, hitbox) in &hitboxes {
                if hitbox.is_hovered(window) {
                    drag.replace(Some(Drag { split: *split, axis: *axis }));
                    cx.stop_propagation();
                    return;
                }
            }
        });

        // Click focuses the pane under the pointer.
        let root = self.root.clone();
        let panes = frame.panes.clone();
        let focused = self.focused;
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            for (pane, bounds) in &panes {
                if bounds.contains(&event.position) {
                    if *pane != focused {
                        let pane = *pane;
                        root.update(cx, |this, cx| this.focuspane(pane, window, cx)).ok();
                    }
                    return;
                }
            }
        });

        // Divider drag adjusts the split ratio.
        let drag = self.drag.clone();
        let root = self.root.clone();
        let tree = self.tree.clone();
        let rect = frame.rect;
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _window, cx| {
            if phase != DispatchPhase::Bubble {
                return;
            }
            let Some(d) = *drag.borrow() else {
                return;
            };
            let Some(srect) = splitrect(&tree, rect, DIVIDER, d.split) else {
                return;
            };
            let ratio = dragratio(
                srect,
                d.axis,
                DIVIDER,
                (f32::from(event.position.x), f32::from(event.position.y)),
            );
            root.update(cx, |this, cx| this.setratio(d.split, ratio, cx)).ok();
        });

        // Release ends the drag.
        let drag = self.drag.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, _window, _cx| {
            if phase == DispatchPhase::Bubble {
                drag.replace(None);
            }
        });
    }
}

impl IntoElement for SplitsElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use workspace::PaneIds;

    fn ids(n: usize) -> Vec<PaneId> {
        let mut alloc = PaneIds::new();
        (0..n).map(|_| alloc.next()).collect()
    }

    #[test]
    fn splitrect_root_is_full_rect() {
        let p = ids(2);
        let mut tree = PaneTree::new(p[0]);
        let s = tree.split(p[0], Axis::Horizontal, p[1], false).unwrap();
        let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(splitrect(&tree, rect, 2.0, s), Some(rect));
    }

    #[test]
    fn splitrect_nested_matches_compute_layout() {
        // a | (b over c): the inner split spans exactly b + divider + c.
        let p = ids(3);
        let mut tree = PaneTree::new(p[0]);
        let outer = tree.split(p[0], Axis::Horizontal, p[1], false).unwrap();
        let inner = tree.split(p[1], Axis::Vertical, p[2], false).unwrap();
        let rect = Rect::new(0.0, 0.0, 104.0, 104.0);
        let layout = compute_layout(&tree, rect, 4.0);

        assert_eq!(splitrect(&tree, rect, 4.0, outer), Some(rect));
        let irect = splitrect(&tree, rect, 4.0, inner).expect("inner rect");
        let b = layout.pane_rect(p[1]).unwrap();
        let c = layout.pane_rect(p[2]).unwrap();
        assert_eq!((irect.x, irect.y), (b.x, b.y));
        assert_eq!(irect.w, b.w);
        assert_eq!(irect.h, b.h + 4.0 + c.h);
        // The inner divider sits inside the inner split's rect.
        let (_, drect, _) = layout.dividers[1];
        assert!(drect.y >= irect.y && drect.y + drect.h <= irect.y + irect.h);
    }

    #[test]
    fn splitrect_honors_nondefault_ratio() {
        let p = ids(3);
        let mut tree = PaneTree::new(p[0]);
        let outer = tree.split(p[0], Axis::Horizontal, p[1], false).unwrap();
        tree.set_ratio(outer, 0.25);
        let inner = tree.split(p[1], Axis::Vertical, p[2], false).unwrap();
        let rect = Rect::new(0.0, 0.0, 102.0, 60.0);
        // avail = 100, first = 25, divider 2: inner starts at x = 27.
        let irect = splitrect(&tree, rect, 2.0, inner).expect("inner");
        assert_eq!(irect, Rect::new(27.0, 0.0, 75.0, 60.0));
    }

    #[test]
    fn splitrect_missing_split_is_none() {
        // Mint an id the queried trees never allocated: a second split in
        // a throwaway tree (per-tree counters make it id 2).
        let p = ids(3);
        let mut donor = PaneTree::new(p[0]);
        donor.split(p[0], Axis::Horizontal, p[1], false).unwrap();
        let foreign = donor.split(p[1], Axis::Vertical, p[2], false).unwrap();

        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);
        let mut tree = PaneTree::new(p[0]);
        tree.split(p[0], Axis::Horizontal, p[1], false).unwrap();
        assert_eq!(splitrect(&tree, rect, 1.0, foreign), None);
        let single = PaneTree::new(p[0]);
        assert_eq!(splitrect(&single, rect, 1.0, foreign), None);
    }

    #[test]
    fn dragratio_tracks_pointer_horizontally() {
        let rect = Rect::new(0.0, 0.0, 102.0, 50.0);
        // Pointer at the divider center for ratio 0.5: x = 50 + 1.
        let r = dragratio(rect, Axis::Horizontal, 2.0, (51.0, 0.0));
        assert!((r - 0.5).abs() < 1e-6);
        let r = dragratio(rect, Axis::Horizontal, 2.0, (26.0, 999.0));
        assert!((r - 0.25).abs() < 1e-6);
    }

    #[test]
    fn dragratio_vertical_uses_y_and_offset() {
        let rect = Rect::new(0.0, 100.0, 80.0, 102.0);
        let r = dragratio(rect, Axis::Vertical, 2.0, (0.0, 151.0));
        assert!((r - 0.5).abs() < 1e-6);
    }

    #[test]
    fn dragratio_clamps_to_bounds() {
        let rect = Rect::new(0.0, 0.0, 102.0, 50.0);
        assert_eq!(dragratio(rect, Axis::Horizontal, 2.0, (-50.0, 0.0)), 0.1);
        assert_eq!(dragratio(rect, Axis::Horizontal, 2.0, (500.0, 0.0)), 0.9);
    }
}
