use crate::types::{Gaps, Rect, WindowId};

/// Monocle layout -- one window visible at a time, carousel navigation.
///
/// Windows are stored in insertion order. Exactly one window is focused at a
/// time (the "visible" one). Navigation wraps around in both directions.
#[derive(Debug)]
pub struct MonocleLayout {
    windows: Vec<WindowId>,
    focused_index: Option<usize>,
}

impl MonocleLayout {
    /// Create an empty monocle layout.
    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            focused_index: None,
        }
    }

    /// Add a window to the end of the layout. The new window becomes focused.
    pub fn add_window(&mut self, id: WindowId) {
        self.windows.push(id);
        self.focused_index = Some(self.windows.len() - 1);
    }

    /// Remove a window from the layout.
    ///
    /// If the removed window was focused, promotes the next window (or the
    /// previous one if the removed window was last in the list).
    /// Returns the new focused window, or `None` if the layout is now empty.
    pub fn remove_window(&mut self, id: WindowId) -> Option<WindowId> {
        let pos = self.windows.iter().position(|w| *w == id)?;
        self.windows.remove(pos);

        if self.windows.is_empty() {
            self.focused_index = None;
            return None;
        }

        // Only need to update focus if we removed the focused window or a
        // window before it shifted indices.
        if let Some(fi) = self.focused_index {
            if pos == fi {
                // Removed the focused window. Promote next; if at end, wrap to prev.
                self.focused_index = Some(if pos < self.windows.len() {
                    pos
                } else {
                    self.windows.len() - 1
                });
            } else if pos < fi {
                // Removed before focused — shift index left.
                self.focused_index = Some(fi - 1);
            }
            // pos > fi: focused index unchanged.
        }

        self.focused()
    }

    /// Move focus to the next window (wrapping carousel forward).
    /// Returns the newly focused window, or `None` if the layout is empty.
    pub fn focus_next(&mut self) -> Option<WindowId> {
        let fi = self.focused_index?;
        let next = (fi + 1) % self.windows.len();
        self.focused_index = Some(next);
        Some(self.windows[next])
    }

    /// Move focus to the previous window (wrapping carousel backward).
    /// Returns the newly focused window, or `None` if the layout is empty.
    pub fn focus_prev(&mut self) -> Option<WindowId> {
        let fi = self.focused_index?;
        let prev = if fi == 0 {
            self.windows.len() - 1
        } else {
            fi - 1
        };
        self.focused_index = Some(prev);
        Some(self.windows[prev])
    }

    /// The currently focused window, or `None` if the layout is empty.
    pub fn focused(&self) -> Option<WindowId> {
        self.focused_index.map(|i| self.windows[i])
    }

    /// All windows in insertion order.
    pub fn windows(&self) -> &[WindowId] {
        &self.windows
    }

    /// Number of windows in the layout.
    pub fn len(&self) -> usize {
        self.windows.len()
    }

    /// Whether the layout has no windows.
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Calculate the position for every window in the layout.
    ///
    /// The focused window gets the screen rect adjusted for gaps (or the full
    /// screen rect if `fullscreen` is true). All other windows are moved to an
    /// offscreen position so they are invisible but not destroyed.
    pub fn calculate_positions(
        &self,
        screen: Rect,
        gaps: Gaps,
        fullscreen: bool,
    ) -> Vec<(WindowId, Rect)> {
        let focused_id = self.focused();

        self.windows
            .iter()
            .map(|&id| {
                if Some(id) == focused_id {
                    let rect = if fullscreen {
                        screen
                    } else {
                        Rect {
                            x: screen.x + gaps.outer,
                            y: screen.y + gaps.outer,
                            width: screen.width - 2.0 * gaps.outer,
                            height: screen.height - 2.0 * gaps.outer,
                        }
                    };
                    (id, rect)
                } else {
                    // Offscreen: far enough that no pixel leaks.
                    let offscreen = Rect {
                        x: screen.x + 10000.0,
                        y: screen.y + 10000.0,
                        width: screen.width,
                        height: screen.height,
                    };
                    (id, offscreen)
                }
            })
            .collect()
    }
}

impl Default for MonocleLayout {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wid(n: u32) -> WindowId {
        WindowId(n)
    }

    fn screen() -> Rect {
        Rect {
            x: 0.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        }
    }

    fn gaps() -> Gaps {
        Gaps {
            inner: 8.0,
            outer: 10.0,
        }
    }

    #[test]
    fn test_empty_monocle() {
        let layout = MonocleLayout::new();
        assert!(layout.is_empty());
        assert_eq!(layout.len(), 0);
        assert_eq!(layout.focused(), None);
        assert!(layout.windows().is_empty());
    }

    #[test]
    fn test_add_single_window() {
        let mut layout = MonocleLayout::new();
        layout.add_window(wid(1));
        assert_eq!(layout.len(), 1);
        assert_eq!(layout.focused(), Some(wid(1)));
    }

    #[test]
    fn test_add_multiple_windows() {
        let mut layout = MonocleLayout::new();
        layout.add_window(wid(1));
        layout.add_window(wid(2));
        layout.add_window(wid(3));
        assert_eq!(layout.len(), 3);
        // Last added window is focused.
        assert_eq!(layout.focused(), Some(wid(3)));
    }

    #[test]
    fn test_focus_next_wraps() {
        // AC-03: A,B,C focused=C -> focus_next -> A
        let mut layout = MonocleLayout::new();
        layout.add_window(wid(1));
        layout.add_window(wid(2));
        layout.add_window(wid(3));
        // focused is 3 (last added)
        assert_eq!(layout.focused(), Some(wid(3)));
        let next = layout.focus_next();
        assert_eq!(next, Some(wid(1)));
        assert_eq!(layout.focused(), Some(wid(1)));
    }

    #[test]
    fn test_focus_prev_wraps() {
        // A,B,C focused=A -> focus_prev -> C
        let mut layout = MonocleLayout::new();
        layout.add_window(wid(1));
        layout.add_window(wid(2));
        layout.add_window(wid(3));
        // Move focus to first window.
        layout.focus_next(); // 3 -> 1
        assert_eq!(layout.focused(), Some(wid(1)));
        let prev = layout.focus_prev();
        assert_eq!(prev, Some(wid(3)));
    }

    #[test]
    fn test_focus_next_sequential() {
        // AC-02: A,B,C focused=A -> focus_next -> B
        let mut layout = MonocleLayout::new();
        layout.add_window(wid(1));
        layout.add_window(wid(2));
        layout.add_window(wid(3));
        // Move focus to A first.
        layout.focus_next(); // 3 -> 1
        assert_eq!(layout.focused(), Some(wid(1)));
        let next = layout.focus_next();
        assert_eq!(next, Some(wid(2)));
    }

    #[test]
    fn test_remove_focused_promotes_next() {
        // AC-11: A(f),B,C -> remove A -> B focused
        let mut layout = MonocleLayout::new();
        layout.add_window(wid(1));
        layout.add_window(wid(2));
        layout.add_window(wid(3));
        // Focus window 1.
        layout.focus_next(); // 3 -> 1
        assert_eq!(layout.focused(), Some(wid(1)));
        let new_focus = layout.remove_window(wid(1));
        assert_eq!(new_focus, Some(wid(2)));
        assert_eq!(layout.windows(), &[wid(2), wid(3)]);
    }

    #[test]
    fn test_remove_focused_last_promotes_prev() {
        // A,B,C(f) -> remove C -> B focused
        let mut layout = MonocleLayout::new();
        layout.add_window(wid(1));
        layout.add_window(wid(2));
        layout.add_window(wid(3));
        // focused is 3 (last added, which is at the end)
        assert_eq!(layout.focused(), Some(wid(3)));
        let new_focus = layout.remove_window(wid(3));
        assert_eq!(new_focus, Some(wid(2)));
        assert_eq!(layout.windows(), &[wid(1), wid(2)]);
    }

    #[test]
    fn test_remove_only_window() {
        let mut layout = MonocleLayout::new();
        layout.add_window(wid(1));
        let new_focus = layout.remove_window(wid(1));
        assert_eq!(new_focus, None);
        assert!(layout.is_empty());
        assert_eq!(layout.focused(), None);
    }

    #[test]
    fn test_remove_nonfocused() {
        // A(f),B,C -> remove B -> A still focused, list=[A,C]
        let mut layout = MonocleLayout::new();
        layout.add_window(wid(1));
        layout.add_window(wid(2));
        layout.add_window(wid(3));
        // Focus window 1.
        layout.focus_next(); // 3 -> 1
        assert_eq!(layout.focused(), Some(wid(1)));
        let new_focus = layout.remove_window(wid(2));
        assert_eq!(new_focus, Some(wid(1)));
        assert_eq!(layout.windows(), &[wid(1), wid(3)]);
    }

    #[test]
    fn test_calculate_positions_gaps() {
        let mut layout = MonocleLayout::new();
        layout.add_window(wid(1));
        layout.add_window(wid(2));
        layout.add_window(wid(3));
        // focused = 3 (last added)

        let positions = layout.calculate_positions(screen(), gaps(), false);
        assert_eq!(positions.len(), 3);

        // Window 3 (focused) gets gapped rect.
        let (id, rect) = positions.iter().find(|(id, _)| *id == wid(3)).unwrap();
        assert_eq!(*id, wid(3));
        assert_eq!(rect.x, 10.0); // outer gap
        assert_eq!(rect.y, 10.0);
        assert_eq!(rect.width, 1900.0); // 1920 - 2*10
        assert_eq!(rect.height, 1060.0); // 1080 - 2*10

        // Other windows are offscreen.
        for &check_id in &[wid(1), wid(2)] {
            let (_, rect) = positions.iter().find(|(id, _)| *id == check_id).unwrap();
            assert_eq!(rect.x, 10000.0);
            assert_eq!(rect.y, 10000.0);
        }
    }

    #[test]
    fn test_calculate_positions_fullscreen() {
        let mut layout = MonocleLayout::new();
        layout.add_window(wid(1));
        layout.add_window(wid(2));

        let positions = layout.calculate_positions(screen(), gaps(), true);
        assert_eq!(positions.len(), 2);

        // Focused window (2) gets full screen rect, zero gaps.
        let (_, rect) = positions.iter().find(|(id, _)| *id == wid(2)).unwrap();
        assert_eq!(rect.x, 0.0);
        assert_eq!(rect.y, 0.0);
        assert_eq!(rect.width, 1920.0);
        assert_eq!(rect.height, 1080.0);

        // Other window offscreen.
        let (_, rect) = positions.iter().find(|(id, _)| *id == wid(1)).unwrap();
        assert_eq!(rect.x, 10000.0);
        assert_eq!(rect.y, 10000.0);
    }
}
