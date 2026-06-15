//! Binary split tree of panes.

use crate::id::{PaneId, SplitId};

/// Direction panes are laid out along within a split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    /// Side by side: `first` is left, `second` is right. Divider is vertical.
    Horizontal,
    /// Stacked: `first` is top, `second` is bottom. Divider is horizontal.
    Vertical,
}

pub const MIN_RATIO: f32 = 0.1;
pub const MAX_RATIO: f32 = 0.9;

/// Clamp a split ratio into the allowed `0.1..=0.9` range.
pub fn clamp_ratio(ratio: f32) -> f32 {
    ratio.clamp(MIN_RATIO, MAX_RATIO)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Leaf(PaneId),
    Split {
        id: SplitId,
        axis: Axis,
        /// Fraction of the available space given to `first`. Always clamped.
        ratio: f32,
        first: Box<Node>,
        second: Box<Node>,
    },
}

/// A tree of panes. Always contains at least one pane.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneTree {
    root: Node,
    splits: u64,
}

impl PaneTree {
    pub fn new(root: PaneId) -> Self {
        Self { root: Node::Leaf(root), splits: 0 }
    }

    pub fn root(&self) -> &Node {
        &self.root
    }

    /// Split the leaf holding `target`, placing `new_pane` beside it.
    /// `new_first` puts the new pane left/top. Ratio starts at 0.5.
    /// Returns the id of the created split, or `None` if `target` is absent
    /// or `new_pane` is already present.
    pub fn split(
        &mut self,
        target: PaneId,
        axis: Axis,
        new_pane: PaneId,
        new_first: bool,
    ) -> Option<SplitId> {
        if !self.contains(target) || self.contains(new_pane) {
            return None;
        }
        self.splits += 1;
        let id = SplitId(self.splits);
        splitnode(&mut self.root, target, axis, new_pane, new_first, id);
        Some(id)
    }

    /// Remove a pane, collapsing its parent split into the sibling subtree.
    /// Returns `false` if the pane is absent or is the last pane.
    pub fn remove(&mut self, pane: PaneId) -> bool {
        removenode(&mut self.root, pane)
    }

    /// Set a divider's ratio (clamped to `0.1..=0.9`). `false` if `split` is absent.
    pub fn set_ratio(&mut self, split: SplitId, ratio: f32) -> bool {
        setrationode(&mut self.root, split, clamp_ratio(ratio))
    }

    /// Current ratio of a split, if it exists.
    pub fn ratio(&self, split: SplitId) -> Option<f32> {
        rationode(&self.root, split)
    }

    /// All dividers in layout order (depth first, parent before children).
    pub fn list_dividers(&self) -> Vec<(SplitId, Axis)> {
        let mut out = Vec::new();
        dividers(&self.root, &mut out);
        out
    }

    /// All panes in layout order (left/top before right/bottom).
    pub fn panes(&self) -> Vec<PaneId> {
        let mut out = Vec::new();
        leaves(&self.root, &mut out);
        out
    }

    pub fn contains(&self, pane: PaneId) -> bool {
        containsnode(&self.root, pane)
    }
}

fn splitnode(
    node: &mut Node,
    target: PaneId,
    axis: Axis,
    new_pane: PaneId,
    new_first: bool,
    id: SplitId,
) -> bool {
    match node {
        Node::Leaf(pane) if *pane == target => {
            let (first, second) = if new_first {
                (Node::Leaf(new_pane), Node::Leaf(target))
            } else {
                (Node::Leaf(target), Node::Leaf(new_pane))
            };
            *node = Node::Split {
                id,
                axis,
                ratio: 0.5,
                first: Box::new(first),
                second: Box::new(second),
            };
            true
        }
        Node::Leaf(_) => false,
        Node::Split { first, second, .. } => {
            splitnode(first, target, axis, new_pane, new_first, id)
                || splitnode(second, target, axis, new_pane, new_first, id)
        }
    }
}

fn removenode(node: &mut Node, pane: PaneId) -> bool {
    let Node::Split { first, second, .. } = node else {
        return false;
    };
    let in_first = matches!(first.as_ref(), Node::Leaf(p) if *p == pane);
    let in_second = matches!(second.as_ref(), Node::Leaf(p) if *p == pane);
    if in_first || in_second {
        let keep = if in_first { second } else { first };
        *node = std::mem::replace(keep.as_mut(), Node::Leaf(pane));
        return true;
    }
    removenode(first, pane) || removenode(second, pane)
}

fn setrationode(node: &mut Node, split: SplitId, clamped: f32) -> bool {
    let Node::Split { id, ratio, first, second, .. } = node else {
        return false;
    };
    if *id == split {
        *ratio = clamped;
        return true;
    }
    setrationode(first, split, clamped) || setrationode(second, split, clamped)
}

fn rationode(node: &Node, split: SplitId) -> Option<f32> {
    let Node::Split { id, ratio, first, second, .. } = node else {
        return None;
    };
    if *id == split {
        return Some(*ratio);
    }
    rationode(first, split).or_else(|| rationode(second, split))
}

fn dividers(node: &Node, out: &mut Vec<(SplitId, Axis)>) {
    if let Node::Split { id, axis, first, second, .. } = node {
        out.push((*id, *axis));
        dividers(first, out);
        dividers(second, out);
    }
}

fn leaves(node: &Node, out: &mut Vec<PaneId>) {
    match node {
        Node::Leaf(pane) => out.push(*pane),
        Node::Split { first, second, .. } => {
            leaves(first, out);
            leaves(second, out);
        }
    }
}

fn containsnode(node: &Node, pane: PaneId) -> bool {
    match node {
        Node::Leaf(p) => *p == pane,
        Node::Split { first, second, .. } => {
            containsnode(first, pane) || containsnode(second, pane)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::PaneIds;

    fn ids(n: usize) -> Vec<PaneId> {
        let mut alloc = PaneIds::new();
        (0..n).map(|_| alloc.next()).collect()
    }

    #[test]
    fn new_tree_holds_root() {
        let p = ids(1);
        let tree = PaneTree::new(p[0]);
        assert_eq!(tree.panes(), vec![p[0]]);
        assert!(tree.contains(p[0]));
        assert!(tree.list_dividers().is_empty());
    }

    #[test]
    fn split_orders_panes() {
        let p = ids(3);
        let mut tree = PaneTree::new(p[0]);
        tree.split(p[0], Axis::Horizontal, p[1], false).unwrap();
        assert_eq!(tree.panes(), vec![p[0], p[1]]);
        // new_first puts the new pane before the target.
        tree.split(p[0], Axis::Vertical, p[2], true).unwrap();
        assert_eq!(tree.panes(), vec![p[2], p[0], p[1]]);
    }

    #[test]
    fn split_rejects_missing_target_and_duplicate_pane() {
        let p = ids(3);
        let mut tree = PaneTree::new(p[0]);
        assert!(tree.split(p[1], Axis::Horizontal, p[2], false).is_none());
        assert!(tree.split(p[0], Axis::Horizontal, p[0], false).is_none());
        assert_eq!(tree.panes(), vec![p[0]]);
    }

    #[test]
    fn remove_collapses_chain() {
        let p = ids(3);
        let mut tree = PaneTree::new(p[0]);
        tree.split(p[0], Axis::Horizontal, p[1], false).unwrap();
        tree.split(p[1], Axis::Vertical, p[2], false).unwrap();
        assert_eq!(tree.panes(), vec![p[0], p[1], p[2]]);

        assert!(tree.remove(p[1]));
        assert_eq!(tree.panes(), vec![p[0], p[2]]);
        assert_eq!(tree.list_dividers().len(), 1);

        assert!(tree.remove(p[2]));
        assert_eq!(tree.panes(), vec![p[0]]);
        assert_eq!(tree.root(), &Node::Leaf(p[0]));
    }

    #[test]
    fn remove_inner_split_sibling_subtree_survives() {
        let p = ids(4);
        let mut tree = PaneTree::new(p[0]);
        tree.split(p[0], Axis::Horizontal, p[1], false).unwrap();
        tree.split(p[0], Axis::Vertical, p[2], false).unwrap();
        tree.split(p[2], Axis::Vertical, p[3], false).unwrap();
        assert_eq!(tree.panes(), vec![p[0], p[2], p[3], p[1]]);

        assert!(tree.remove(p[0]));
        assert_eq!(tree.panes(), vec![p[2], p[3], p[1]]);
        assert!(tree.remove(p[3]));
        assert!(tree.remove(p[1]));
        assert_eq!(tree.panes(), vec![p[2]]);
    }

    #[test]
    fn remove_refuses_last_pane_and_missing() {
        let p = ids(2);
        let mut tree = PaneTree::new(p[0]);
        assert!(!tree.remove(p[0]));
        assert!(!tree.remove(p[1]));
        assert!(tree.contains(p[0]));
    }

    #[test]
    fn ratio_clamps() {
        let p = ids(2);
        let mut tree = PaneTree::new(p[0]);
        let s = tree.split(p[0], Axis::Horizontal, p[1], false).unwrap();
        assert_eq!(tree.ratio(s), Some(0.5));
        assert!(tree.set_ratio(s, 0.05));
        assert_eq!(tree.ratio(s), Some(MIN_RATIO));
        assert!(tree.set_ratio(s, 0.95));
        assert_eq!(tree.ratio(s), Some(MAX_RATIO));
        assert!(tree.set_ratio(s, 0.3));
        assert_eq!(tree.ratio(s), Some(0.3));
        assert!(!tree.set_ratio(SplitId(999), 0.5));
    }

    #[test]
    fn dividers_listed_parent_first() {
        let p = ids(3);
        let mut tree = PaneTree::new(p[0]);
        let outer = tree.split(p[0], Axis::Horizontal, p[1], false).unwrap();
        let inner = tree.split(p[1], Axis::Vertical, p[2], false).unwrap();
        assert_eq!(
            tree.list_dividers(),
            vec![(outer, Axis::Horizontal), (inner, Axis::Vertical)]
        );
    }
}
