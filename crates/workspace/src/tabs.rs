//! Ordered tabs, each owning a pane tree and a focused pane.

use crate::id::PaneId;
use crate::tree::PaneTree;

#[derive(Debug, Clone, PartialEq)]
pub struct Tab {
    pub tree: PaneTree,
    pub focused: PaneId,
    /// User-set tab label; overrides the focused pane's title when present.
    pub title: Option<String>,
}

impl Tab {
    pub fn new(root: PaneId) -> Self {
        Self {
            tree: PaneTree::new(root),
            focused: root,
            title: None,
        }
    }
}

/// Ordered tabs with one active. Always holds at least one tab.
#[derive(Debug, Clone, PartialEq)]
pub struct Tabs {
    tabs: Vec<Tab>,
    active: usize,
}

impl Tabs {
    pub fn new(root: PaneId) -> Self {
        Self {
            tabs: vec![Tab::new(root)],
            active: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active(&self) -> &Tab {
        &self.tabs[self.active]
    }

    pub fn active_mut(&mut self) -> &mut Tab {
        &mut self.tabs[self.active]
    }

    pub fn get(&self, index: usize) -> Option<&Tab> {
        self.tabs.get(index)
    }

    /// Append a tab rooted at `root` and activate it. Returns its index.
    pub fn new_tab(&mut self, root: PaneId) -> usize {
        self.tabs.push(Tab::new(root));
        self.active = self.tabs.len() - 1;
        self.active
    }

    /// Close the tab at `index`. `false` if out of range or it is the last tab.
    pub fn close_tab(&mut self, index: usize) -> bool {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(index);
        if index < self.active {
            self.active -= 1;
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        true
    }

    /// `false` if `index` is out of range.
    pub fn activate(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }
        self.active = index;
        true
    }

    pub fn activate_next(&mut self) {
        self.active = (self.active + 1) % self.tabs.len();
    }

    pub fn activate_prev(&mut self) {
        self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
    }

    /// Move the tab at `from` to position `to`, keeping the active tab active.
    /// `false` if either index is out of range.
    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        let len = self.tabs.len();
        if from >= len || to >= len {
            return false;
        }
        if from == to {
            return true;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
        if self.active == from {
            self.active = to;
        } else if from < self.active && to >= self.active {
            self.active -= 1;
        } else if from > self.active && to <= self.active {
            self.active += 1;
        }
        true
    }

    /// Focus a pane in the active tab. `false` if the pane is not in its tree.
    pub fn focus(&mut self, pane: PaneId) -> bool {
        if !self.active().tree.contains(pane) {
            return false;
        }
        self.active_mut().focused = pane;
        true
    }

    /// The focused pane of the active tab.
    pub fn focused(&self) -> PaneId {
        self.active().focused
    }

    /// Override the label of the tab at `index` (empty/None reverts to the
    /// focused pane's title). `false` if `index` is out of range.
    pub fn set_title(&mut self, index: usize, title: Option<String>) -> bool {
        match self.tabs.get_mut(index) {
            Some(tab) => {
                tab.title = title;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::PaneIds;
    use crate::tree::Axis;

    fn ids(n: usize) -> Vec<PaneId> {
        let mut alloc = PaneIds::new();
        (0..n).map(|_| alloc.next()).collect()
    }

    #[test]
    fn starts_with_one_active_tab() {
        let p = ids(1);
        let tabs = Tabs::new(p[0]);
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.focused(), p[0]);
        assert!(!tabs.is_empty());
    }

    #[test]
    fn new_tab_appends_and_activates() {
        let p = ids(3);
        let mut tabs = Tabs::new(p[0]);
        assert_eq!(tabs.new_tab(p[1]), 1);
        assert_eq!(tabs.new_tab(p[2]), 2);
        assert_eq!(tabs.active_index(), 2);
        assert_eq!(tabs.focused(), p[2]);
    }

    #[test]
    fn close_tab_refuses_last_and_out_of_range() {
        let p = ids(2);
        let mut tabs = Tabs::new(p[0]);
        assert!(!tabs.close_tab(0));
        tabs.new_tab(p[1]);
        assert!(!tabs.close_tab(5));
        assert!(tabs.close_tab(1));
        assert_eq!(tabs.len(), 1);
        assert!(!tabs.close_tab(0));
    }

    #[test]
    fn close_tab_adjusts_active_index() {
        let p = ids(3);
        let mut tabs = Tabs::new(p[0]);
        tabs.new_tab(p[1]);
        tabs.new_tab(p[2]);

        // Closing before the active tab shifts it left.
        tabs.activate(2);
        assert!(tabs.close_tab(0));
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(tabs.focused(), p[2]);

        // Closing the active last tab clamps to the new end.
        assert!(tabs.close_tab(1));
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.focused(), p[1]);
    }

    #[test]
    fn activate_and_cycling_wrap() {
        let p = ids(3);
        let mut tabs = Tabs::new(p[0]);
        tabs.new_tab(p[1]);
        tabs.new_tab(p[2]);
        assert!(tabs.activate(0));
        assert!(!tabs.activate(3));
        assert_eq!(tabs.active_index(), 0);
        tabs.activate_prev();
        assert_eq!(tabs.active_index(), 2);
        tabs.activate_next();
        assert_eq!(tabs.active_index(), 0);
        tabs.activate_next();
        assert_eq!(tabs.active_index(), 1);
    }

    #[test]
    fn move_tab_reorders_and_tracks_active() {
        let p = ids(3);
        let mut tabs = Tabs::new(p[0]);
        tabs.new_tab(p[1]);
        tabs.new_tab(p[2]); // order: 0,1,2; active 2.

        // Moving the active tab keeps it active at its new index.
        assert!(tabs.move_tab(2, 0)); // order: 2,0,1.
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.focused(), p[2]);

        // Moving another tab across the active one shifts the index.
        assert!(tabs.move_tab(2, 0)); // order: 1,2,0; active follows p[2] to 1.
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(tabs.focused(), p[2]);
        assert_eq!(tabs.get(0).unwrap().focused, p[1]);
        assert_eq!(tabs.get(2).unwrap().focused, p[0]);

        assert!(!tabs.move_tab(0, 9));
        assert!(!tabs.move_tab(9, 0));
        assert!(tabs.move_tab(1, 1));
    }

    #[test]
    fn focus_only_panes_in_active_tab() {
        let p = ids(3);
        let mut tabs = Tabs::new(p[0]);
        tabs.active_mut()
            .tree
            .split(p[0], Axis::Horizontal, p[1], false)
            .unwrap();
        assert!(tabs.focus(p[1]));
        assert_eq!(tabs.focused(), p[1]);
        assert!(!tabs.focus(p[2]));
        assert_eq!(tabs.focused(), p[1]);

        tabs.new_tab(p[2]);
        assert_eq!(tabs.focused(), p[2]);
        assert!(!tabs.focus(p[0])); // p[0] lives in the other tab.
        tabs.activate(0);
        assert_eq!(tabs.focused(), p[1]); // per-tab focus is remembered.
    }
}
