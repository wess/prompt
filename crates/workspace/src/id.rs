//! Stable opaque identifiers for panes and splits.

/// Identifies a pane. Allocate via [`PaneIds`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PaneId(pub(crate) u64);

/// Identifies a split node (a divider) inside a [`crate::PaneTree`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SplitId(pub(crate) u64);

/// Monotonic [`PaneId`] allocator. Owned by the caller; never reuses ids.
#[derive(Debug, Default, Clone)]
pub struct PaneIds(u64);

impl PaneIds {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next(&mut self) -> PaneId {
        self.0 += 1;
        PaneId(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_monotonic() {
        let mut ids = PaneIds::new();
        let a = ids.next();
        let b = ids.next();
        let c = ids.next();
        assert_ne!(a, b);
        assert_ne!(b, c);
        assert!(a < b && b < c);
    }
}
