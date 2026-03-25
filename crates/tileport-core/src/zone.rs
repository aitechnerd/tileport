#![forbid(unsafe_code)]

use crate::types::{Gaps, Rect, WindowId};
use serde::{Deserialize, Serialize};

/// Direction for focus/move navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

/// A node in the zone definition tree.
#[derive(Debug, Clone)]
pub enum ZoneNode {
    /// A leaf zone that holds windows.
    Leaf,
    /// Horizontal split (side-by-side columns).
    HSplit {
        ratios: Vec<f64>,
        children: Vec<ZoneNode>,
    },
    /// Vertical split (stacked rows).
    VSplit {
        ratios: Vec<f64>,
        children: Vec<ZoneNode>,
    },
}

impl ZoneNode {
    /// Recursively count leaf nodes.
    pub fn leaf_count(&self) -> usize {
        match self {
            ZoneNode::Leaf => 1,
            ZoneNode::HSplit { children, .. } | ZoneNode::VSplit { children, .. } => {
                children.iter().map(|c| c.leaf_count()).sum()
            }
        }
    }

    /// Recursively subdivide a region into leaf rects, applying inner gaps
    /// between siblings. Leaf rects are pushed to `rects` in left-to-right,
    /// top-to-bottom order.
    pub fn calculate_rects(&self, region: Rect, gaps: &Gaps, rects: &mut Vec<Rect>) {
        match self {
            ZoneNode::Leaf => {
                rects.push(region);
            }
            ZoneNode::HSplit { ratios, children } => {
                let total_gaps = gaps.inner * (children.len() as f64 - 1.0);
                let usable_width = region.width - total_gaps;
                let mut x = region.x;

                for (i, (child, &ratio)) in
                    children.iter().zip(ratios.iter()).enumerate()
                {
                    let w = usable_width * ratio;
                    let child_region = Rect {
                        x,
                        y: region.y,
                        width: w,
                        height: region.height,
                    };
                    child.calculate_rects(child_region, gaps, rects);
                    x += w;
                    if i < children.len() - 1 {
                        x += gaps.inner;
                    }
                }
            }
            ZoneNode::VSplit { ratios, children } => {
                let total_gaps = gaps.inner * (children.len() as f64 - 1.0);
                let usable_height = region.height - total_gaps;
                let mut y = region.y;

                for (i, (child, &ratio)) in
                    children.iter().zip(ratios.iter()).enumerate()
                {
                    let h = usable_height * ratio;
                    let child_region = Rect {
                        x: region.x,
                        y,
                        width: region.width,
                        height: h,
                    };
                    child.calculate_rects(child_region, gaps, rects);
                    y += h;
                    if i < children.len() - 1 {
                        y += gaps.inner;
                    }
                }
            }
        }
    }
}

/// Generate the default fill order for a zone tree.
///
/// - 3-child HSplit at root: center-first `[1, 0, 2]`
/// - All other cases: left-to-right / top-to-bottom `[0, 1, ..., N-1]`
pub fn make_fill_order(root: &ZoneNode) -> Vec<usize> {
    let count = root.leaf_count();
    match root {
        ZoneNode::HSplit { children, .. } if children.len() == 3 => {
            // Center-first: center gets first window, then left, then right.
            // Leaf indices follow the tree order (left=0..a, center=a..b, right=b..c).
            let left_leaves = children[0].leaf_count();
            let center_leaves = children[1].leaf_count();

            // Center zone leaf indices first, then left, then right.
            let mut order = Vec::with_capacity(count);
            for i in left_leaves..(left_leaves + center_leaves) {
                order.push(i);
            }
            for i in 0..left_leaves {
                order.push(i);
            }
            for i in (left_leaves + center_leaves)..count {
                order.push(i);
            }
            order
        }
        _ => (0..count).collect(),
    }
}

/// Zone layout engine.
///
/// Windows are distributed across leaf zones according to a fill order.
/// Each zone holds a stack of windows; the first window is visible, the
/// rest are overflow (hidden offscreen). Navigation moves between zones
/// or cycles within a zone's overflow stack.
#[derive(Debug)]
pub struct ZoneLayout {
    /// The zone definition tree.
    root: ZoneNode,
    /// Number of leaf zones.
    zone_count: usize,
    /// Fill order: maps insertion index to zone leaf index.
    fill_order: Vec<usize>,
    /// Windows assigned to each zone. Index = zone leaf index.
    zones: Vec<Vec<WindowId>>,
    /// Index of the currently focused zone.
    focused_zone: usize,
    /// Index of the primary zone (for promote operation).
    primary_zone: usize,
    /// Number of windows added (used to determine next fill_order slot).
    window_count: usize,
}

impl ZoneLayout {
    /// Construct a new zone layout from a parsed zone tree.
    ///
    /// `fill_order` maps insertion index to zone leaf index.
    /// `primary_zone` is the zone index used as the target for promote.
    pub fn new(root: ZoneNode, fill_order: Vec<usize>, primary_zone: usize) -> Self {
        let zone_count = root.leaf_count();

        debug_assert!(
            fill_order.iter().all(|&i| i < zone_count),
            "fill_order contains out-of-bounds index: fill_order={fill_order:?}, zone_count={zone_count}"
        );

        Self {
            root,
            zone_count,
            fill_order,
            zones: vec![Vec::new(); zone_count],
            focused_zone: 0,
            primary_zone,
            window_count: 0,
        }
    }

    /// Add a window to the layout per fill order. The new window becomes focused.
    ///
    /// Finds the first zone in fill order that is empty. If all zones have at
    /// least one window, the new window is pushed as overflow into the last
    /// zone in fill order. This is idempotent across remove/add cycles.
    pub fn add_window(&mut self, id: WindowId) {
        // Find the first empty zone in fill order.
        let zone_idx = self
            .fill_order
            .iter()
            .find(|&&fi| self.zones[fi].is_empty())
            .copied()
            .unwrap_or_else(|| {
                // All zones occupied: overflow to last fill_order zone.
                *self.fill_order.last().unwrap_or(&0)
            });
        self.zones[zone_idx].push(id);
        self.focused_zone = zone_idx;
        self.window_count += 1;
    }

    /// Remove a window from the layout.
    ///
    /// Returns the new focused window, or `None` if the layout is now empty.
    pub fn remove_window(&mut self, id: WindowId) -> Option<WindowId> {
        // Find which zone contains the window.
        let zone_idx = self
            .zones
            .iter()
            .position(|z| z.contains(&id))?;

        let pos = self.zones[zone_idx]
            .iter()
            .position(|w| *w == id)
            .unwrap();
        self.zones[zone_idx].remove(pos);
        self.window_count = self.window_count.saturating_sub(1);

        // C3: Clamp focused_zone to valid range.
        // If the focused zone is now empty, find the nearest non-empty zone,
        // or clamp to zones.len().saturating_sub(1).
        if self.zones[self.focused_zone].is_empty() {
            // Try to find a non-empty zone; prefer staying at current or going down.
            let non_empty = self
                .zones
                .iter()
                .enumerate()
                .find(|(_, z)| !z.is_empty())
                .map(|(i, _)| i);

            match non_empty {
                Some(idx) => self.focused_zone = idx,
                None => {
                    // All zones empty -- clamp to valid index.
                    self.focused_zone = self.zones.len().saturating_sub(1);
                }
            }
        }

        self.focused()
    }

    /// The currently focused window, or `None` if the layout is empty.
    pub fn focused(&self) -> Option<WindowId> {
        self.zones
            .get(self.focused_zone)
            .and_then(|z| z.first().copied())
    }

    /// All windows across all zones, flattened.
    pub fn windows(&self) -> Vec<WindowId> {
        self.zones.iter().flatten().copied().collect()
    }

    /// Number of windows in the layout.
    pub fn len(&self) -> usize {
        self.window_count
    }

    /// Whether the layout has no windows.
    pub fn is_empty(&self) -> bool {
        self.window_count == 0
    }

    /// Calculate the position for every window in the layout.
    ///
    /// Each zone's visible window (index 0) gets the zone rect.
    /// Overflow windows (index 1+) are moved offscreen.
    pub fn calculate_positions(
        &self,
        screen: Rect,
        gaps: Gaps,
        fullscreen: bool,
    ) -> Vec<(WindowId, Rect)> {
        let inner_screen = if fullscreen {
            screen
        } else {
            Rect {
                x: screen.x + gaps.outer,
                y: screen.y + gaps.outer,
                width: screen.width - 2.0 * gaps.outer,
                height: screen.height - 2.0 * gaps.outer,
            }
        };

        let mut zone_rects = Vec::with_capacity(self.zone_count);
        self.root
            .calculate_rects(inner_screen, &gaps, &mut zone_rects);

        let focused_id = self.focused();
        let mut positions = Vec::new();

        for (zone_idx, zone_windows) in self.zones.iter().enumerate() {
            for (win_idx, &wid) in zone_windows.iter().enumerate() {
                let rect = if fullscreen && Some(wid) == focused_id {
                    // Fullscreen focused window gets raw screen rect.
                    screen
                } else if win_idx == 0 {
                    // Visible window gets zone rect.
                    zone_rects[zone_idx]
                } else {
                    // Overflow: offscreen.
                    Rect {
                        x: screen.x + 10000.0,
                        y: screen.y + 10000.0,
                        width: screen.width,
                        height: screen.height,
                    }
                };
                positions.push((wid, rect));
            }
        }

        positions
    }

    /// Move focus to the adjacent zone in the given direction.
    ///
    /// Uses rect-geometry adjacency: computes all leaf rects and finds the
    /// nearest zone whose edge touches the current zone's edge with
    /// sufficient overlap.
    ///
    /// Returns the newly focused window, or `None` if the layout is empty
    /// or there is no adjacent zone in that direction.
    pub fn focus_direction(&mut self, dir: Direction) -> Option<WindowId> {
        if self.is_empty() {
            return None;
        }

        let adj = self.find_adjacent_zone(self.focused_zone, dir);
        if let Some(target) = adj {
            if !self.zones[target].is_empty() {
                self.focused_zone = target;
            }
        } else {
            // No adjacent zone in this direction: fall back to cycling
            // the overflow stack within the current zone, so overflow
            // windows remain reachable via directional keys.
            match dir {
                Direction::Down | Direction::Right => {
                    self.focus_next();
                }
                Direction::Up | Direction::Left => {
                    self.focus_prev();
                }
            }
        }
        self.focused()
    }

    /// Move the focused window to the adjacent zone in the given direction.
    ///
    /// Swaps the visible windows between the focused zone and the target zone.
    /// Returns `true` if a move occurred, `false` for boundary no-ops.
    pub fn move_to_zone(&mut self, dir: Direction) -> bool {
        if self.is_empty() {
            return false;
        }

        let adj = self.find_adjacent_zone(self.focused_zone, dir);
        let target = match adj {
            Some(t) => t,
            None => return false,
        };

        let src = self.focused_zone;

        // Swap visible windows (index 0) between zones.
        let src_win = self.zones[src].first().copied();
        let dst_win = self.zones[target].first().copied();

        match (src_win, dst_win) {
            (Some(sw), Some(dw)) => {
                // Both zones have windows: swap them.
                self.zones[src][0] = dw;
                self.zones[target][0] = sw;
            }
            (Some(sw), None) => {
                // Target is empty: move window there.
                self.zones[src].remove(0);
                self.zones[target].push(sw);
            }
            _ => return false,
        }

        self.focused_zone = target;
        true
    }

    /// Swap the focused window with the primary zone's visible window.
    ///
    /// Returns `true` if a swap occurred, `false` if already primary or
    /// either zone is empty.
    pub fn promote_to_primary(&mut self) -> bool {
        if self.focused_zone == self.primary_zone {
            return false;
        }

        let src = self.focused_zone;
        let dst = self.primary_zone;

        let src_win = self.zones[src].first().copied();
        let dst_win = self.zones[dst].first().copied();

        match (src_win, dst_win) {
            (Some(sw), Some(dw)) => {
                self.zones[src][0] = dw;
                self.zones[dst][0] = sw;
                self.focused_zone = dst;
                true
            }
            (Some(sw), None) => {
                self.zones[src].remove(0);
                self.zones[dst].push(sw);
                self.focused_zone = dst;
                true
            }
            _ => false,
        }
    }

    /// Cycle focus to the next window within the current zone's overflow stack.
    ///
    /// Returns the newly focused window, or `None` if the layout is empty.
    pub fn focus_next(&mut self) -> Option<WindowId> {
        let zone = &mut self.zones[self.focused_zone];
        if zone.len() <= 1 {
            return zone.first().copied();
        }
        // Rotate left: move first element to end.
        let first = zone.remove(0);
        zone.push(first);
        zone.first().copied()
    }

    /// Cycle focus to the previous window within the current zone's overflow stack.
    ///
    /// Returns the newly focused window, or `None` if the layout is empty.
    pub fn focus_prev(&mut self) -> Option<WindowId> {
        let zone = &mut self.zones[self.focused_zone];
        if zone.len() <= 1 {
            return zone.first().copied();
        }
        // Rotate right: move last element to front.
        let last = zone.pop().unwrap();
        zone.insert(0, last);
        zone.first().copied()
    }

    /// Find the zone adjacent to `zone_idx` in the given direction using
    /// rect-geometry: compute all leaf rects and find zones whose edge
    /// touches with sufficient vertical/horizontal overlap.
    fn find_adjacent_zone(&self, zone_idx: usize, dir: Direction) -> Option<usize> {
        // Use a dummy screen to compute relative rects -- the adjacency only
        // depends on topology, not actual screen size, but we need real numbers
        // for edge comparison.
        let screen = Rect {
            x: 0.0,
            y: 0.0,
            width: 10000.0,
            height: 10000.0,
        };
        let gaps = Gaps {
            inner: 10.0,
            outer: 0.0,
        };

        let mut rects = Vec::with_capacity(self.zone_count);
        self.root.calculate_rects(screen, &gaps, &mut rects);

        if zone_idx >= rects.len() {
            return None;
        }

        let src = rects[zone_idx];
        let tolerance = 15.0; // slightly larger than inner gap

        let mut best: Option<(usize, f64)> = None;

        for (i, r) in rects.iter().enumerate() {
            if i == zone_idx {
                continue;
            }

            let (edge_match, overlap) = match dir {
                Direction::Left => {
                    // Target's right edge should touch source's left edge.
                    let edge = ((r.x + r.width) - src.x).abs() < tolerance;
                    let overlap = vertical_overlap(&src, r);
                    (edge, overlap)
                }
                Direction::Right => {
                    // Target's left edge should touch source's right edge.
                    let edge = (r.x - (src.x + src.width)).abs() < tolerance;
                    let overlap = vertical_overlap(&src, r);
                    (edge, overlap)
                }
                Direction::Up => {
                    // Target's bottom edge should touch source's top edge.
                    let edge = ((r.y + r.height) - src.y).abs() < tolerance;
                    let overlap = horizontal_overlap(&src, r);
                    (edge, overlap)
                }
                Direction::Down => {
                    // Target's top edge should touch source's bottom edge.
                    let edge = (r.y - (src.y + src.height)).abs() < tolerance;
                    let overlap = horizontal_overlap(&src, r);
                    (edge, overlap)
                }
            };

            if edge_match
                && overlap > 0.0
                && best.is_none_or(|(_, best_overlap)| overlap > best_overlap)
            {
                best = Some((i, overlap));
            }
        }

        best.map(|(i, _)| i)
    }
}

/// Compute vertical overlap between two rects (shared y-range).
fn vertical_overlap(a: &Rect, b: &Rect) -> f64 {
    let top = a.y.max(b.y);
    let bottom = (a.y + a.height).min(b.y + b.height);
    (bottom - top).max(0.0)
}

/// Compute horizontal overlap between two rects (shared x-range).
fn horizontal_overlap(a: &Rect, b: &Rect) -> f64 {
    let left = a.x.max(b.x);
    let right = (a.x + a.width).min(b.x + b.width);
    (right - left).max(0.0)
}

/// Normalize ratios so they sum to 1.0.
///
/// If any ratio is <= 0.0 or NaN, falls back to equal-weight ratios.
/// If ratios sum is not within tolerance of 1.0, normalizes and logs a warning.
pub fn normalize_ratios(ratios: &[f64]) -> Vec<f64> {
    if ratios.is_empty() {
        return vec![];
    }

    // C1: Per-element guard -- reject zero, negative, or NaN ratios.
    if ratios.iter().any(|&r| r <= 0.0 || r.is_nan()) {
        let equal = 1.0 / ratios.len() as f64;
        return vec![equal; ratios.len()];
    }

    let sum: f64 = ratios.iter().sum();
    if sum.is_nan() || sum <= 0.0 {
        let equal = 1.0 / ratios.len() as f64;
        return vec![equal; ratios.len()];
    }

    let tolerance = 0.01;
    if (sum - 1.0).abs() > tolerance {
        // Normalize to sum to 1.0.
        ratios.iter().map(|&r| r / sum).collect()
    } else {
        ratios.to_vec()
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

    /// Helper: build a 2-column layout with given ratios.
    fn two_col(ratios: [f64; 2]) -> (ZoneNode, Vec<usize>, usize) {
        let root = ZoneNode::HSplit {
            ratios: ratios.to_vec(),
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf],
        };
        let fill_order = make_fill_order(&root);
        (root, fill_order, 0) // primary = fill_order[0] = 0
    }

    /// Helper: build a 3-column layout with given ratios.
    fn three_col(ratios: [f64; 3]) -> (ZoneNode, Vec<usize>, usize) {
        let root = ZoneNode::HSplit {
            ratios: ratios.to_vec(),
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf, ZoneNode::Leaf],
        };
        let fill_order = make_fill_order(&root);
        let primary = fill_order[0]; // center = 1
        (root, fill_order, primary)
    }

    /// Helper: build a 2-row layout with given ratios.
    fn two_row(ratios: [f64; 2]) -> (ZoneNode, Vec<usize>, usize) {
        let root = ZoneNode::VSplit {
            ratios: ratios.to_vec(),
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf],
        };
        let fill_order = make_fill_order(&root);
        (root, fill_order, 0)
    }

    /// Helper: build a composite main-stack (left 50%, right column split into 2 rows 50/50).
    fn composite_main_stack() -> (ZoneNode, Vec<usize>, usize) {
        let right_col = ZoneNode::VSplit {
            ratios: vec![0.5, 0.5],
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf],
        };
        let root = ZoneNode::HSplit {
            ratios: vec![0.5, 0.5],
            children: vec![ZoneNode::Leaf, right_col],
        };
        let fill_order = make_fill_order(&root); // [0, 1, 2]
        (root, fill_order, 0)
    }

    // --- Rect calculation tests ---

    #[test]
    fn test_2col_rect_calculation() {
        // AC-ZT-01: 2-column [0.40, 0.60], screen 1920x1080, gaps inner=8, outer=10.
        let root = ZoneNode::HSplit {
            ratios: vec![0.40, 0.60],
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf],
        };

        // Inner screen after outer gaps:
        let inner = Rect {
            x: 10.0,
            y: 10.0,
            width: 1900.0,  // 1920 - 2*10
            height: 1060.0, // 1080 - 2*10
        };

        let mut rects = Vec::new();
        root.calculate_rects(inner, &gaps(), &mut rects);

        assert_eq!(rects.len(), 2);

        // Usable width = 1900 - 8 (one inner gap) = 1892
        let usable = 1900.0 - 8.0;
        let left_w = usable * 0.40;
        let right_w = usable * 0.60;

        assert_eq!(rects[0].x, 10.0);
        assert_eq!(rects[0].y, 10.0);
        assert!((rects[0].width - left_w).abs() < 0.001);
        assert_eq!(rects[0].height, 1060.0);

        assert!((rects[1].x - (10.0 + left_w + 8.0)).abs() < 0.001);
        assert_eq!(rects[1].y, 10.0);
        assert!((rects[1].width - right_w).abs() < 0.001);
        assert_eq!(rects[1].height, 1060.0);
    }

    #[test]
    fn test_3col_rect_calculation() {
        let root = ZoneNode::HSplit {
            ratios: vec![0.30, 0.40, 0.30],
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf, ZoneNode::Leaf],
        };

        let inner = Rect {
            x: 10.0,
            y: 10.0,
            width: 1900.0,
            height: 1060.0,
        };

        let mut rects = Vec::new();
        root.calculate_rects(inner, &gaps(), &mut rects);

        assert_eq!(rects.len(), 3);

        // Usable width = 1900 - 2*8 = 1884
        let usable = 1900.0 - 16.0;
        let w0 = usable * 0.30;
        let w1 = usable * 0.40;
        let w2 = usable * 0.30;

        assert!((rects[0].width - w0).abs() < 0.001);
        assert!((rects[1].width - w1).abs() < 0.001);
        assert!((rects[2].width - w2).abs() < 0.001);

        // All rects should be at same y and height.
        for r in &rects {
            assert_eq!(r.y, 10.0);
            assert_eq!(r.height, 1060.0);
        }
    }

    #[test]
    fn test_2row_rect_calculation() {
        let root = ZoneNode::VSplit {
            ratios: vec![0.60, 0.40],
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf],
        };

        let inner = Rect {
            x: 10.0,
            y: 10.0,
            width: 1900.0,
            height: 1060.0,
        };

        let mut rects = Vec::new();
        root.calculate_rects(inner, &gaps(), &mut rects);

        assert_eq!(rects.len(), 2);

        // Usable height = 1060 - 8 = 1052
        let usable = 1060.0 - 8.0;
        let top_h = usable * 0.60;
        let bottom_h = usable * 0.40;

        assert_eq!(rects[0].x, 10.0);
        assert_eq!(rects[0].y, 10.0);
        assert_eq!(rects[0].width, 1900.0);
        assert!((rects[0].height - top_h).abs() < 0.001);

        assert_eq!(rects[1].x, 10.0);
        assert!((rects[1].y - (10.0 + top_h + 8.0)).abs() < 0.001);
        assert_eq!(rects[1].width, 1900.0);
        assert!((rects[1].height - bottom_h).abs() < 0.001);
    }

    #[test]
    fn test_composite_main_stack() {
        // AC-ZT-12: left column 50%, right column split into 2 rows 50/50.
        let (root, _, _) = composite_main_stack();

        let inner = Rect {
            x: 10.0,
            y: 10.0,
            width: 1900.0,
            height: 1060.0,
        };

        let mut rects = Vec::new();
        root.calculate_rects(inner, &gaps(), &mut rects);

        assert_eq!(rects.len(), 3);

        // Left column: full height.
        assert_eq!(rects[0].y, 10.0);
        assert_eq!(rects[0].height, 1060.0);

        // Right column is split into two rows; they should share the same x and width.
        assert!((rects[1].x - rects[2].x).abs() < 0.001);
        assert!((rects[1].width - rects[2].width).abs() < 0.001);

        // Top-right should be above bottom-right.
        assert!(rects[1].y < rects[2].y);
    }

    // --- Fill order tests ---

    #[test]
    fn test_3col_center_first_fill() {
        // AC-ZT-02: 3-column center-first fill order.
        let (root, fill_order, primary) = three_col([0.30, 0.40, 0.30]);

        assert_eq!(fill_order, vec![1, 0, 2]); // center, left, right
        assert_eq!(primary, 1); // center is primary

        let mut layout = ZoneLayout::new(root, fill_order, primary);
        layout.add_window(wid(1)); // goes to zone 1 (center)
        layout.add_window(wid(2)); // goes to zone 0 (left)
        layout.add_window(wid(3)); // goes to zone 2 (right)

        assert_eq!(layout.zones[1], vec![wid(1)]); // center
        assert_eq!(layout.zones[0], vec![wid(2)]); // left
        assert_eq!(layout.zones[2], vec![wid(3)]); // right
    }

    #[test]
    fn test_2col_ltr_fill() {
        // AC-ZT-03: 2-column left-to-right fill.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);

        assert_eq!(fill_order, vec![0, 1]); // left, right
        assert_eq!(primary, 0);

        let mut layout = ZoneLayout::new(root, fill_order, primary);
        layout.add_window(wid(1)); // zone 0 (left)
        layout.add_window(wid(2)); // zone 1 (right)

        assert_eq!(layout.zones[0], vec![wid(1)]);
        assert_eq!(layout.zones[1], vec![wid(2)]);
    }

    // --- Overflow tests ---

    #[test]
    fn test_overflow_stacking() {
        // AC-ZT-04: 3 windows in 2-zone layout; third goes to overflow.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0
        layout.add_window(wid(2)); // zone 1
        layout.add_window(wid(3)); // overflow: zone 1

        assert_eq!(layout.zones[0], vec![wid(1)]);
        assert_eq!(layout.zones[1], vec![wid(2), wid(3)]);
        assert_eq!(layout.len(), 3);
    }

    #[test]
    fn test_overflow_promotion_on_remove() {
        // Remove visible window in zone with overflow; overflow promotes.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1));
        layout.add_window(wid(2));
        layout.add_window(wid(3)); // overflow in zone 1

        // Focus zone 1.
        layout.focused_zone = 1;

        // Remove wid(2) -- visible in zone 1.
        let new_focus = layout.remove_window(wid(2));
        assert_eq!(new_focus, Some(wid(3))); // overflow promoted
        assert_eq!(layout.zones[1], vec![wid(3)]);
    }

    #[test]
    fn test_remove_window_from_non_overflow_zone() {
        // Remove the only window in a zone.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0
        layout.add_window(wid(2)); // zone 1

        // Focus zone 0.
        layout.focused_zone = 0;

        let new_focus = layout.remove_window(wid(1));
        // Zone 0 is now empty; focus should move to zone 1.
        assert_eq!(new_focus, Some(wid(2)));
        assert!(layout.zones[0].is_empty());
    }

    // --- C3: focused_zone clamping on removal ---

    #[test]
    fn test_remove_only_window_from_focused_zone_no_panic() {
        // C3: remove the only window from the focused zone, assert no panic.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0
        layout.focused_zone = 0;

        let result = layout.remove_window(wid(1));
        assert_eq!(result, None); // no windows remain
        // focused_zone should be clamped, not panic.
        assert!(layout.focused_zone < layout.zones.len());
    }

    #[test]
    fn test_remove_all_windows_no_panic() {
        // C3: remove all windows; layout should be safe.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1));
        layout.add_window(wid(2));

        layout.remove_window(wid(1));
        layout.remove_window(wid(2));

        assert!(layout.is_empty());
        assert_eq!(layout.focused(), None);
        // Should not panic.
        assert!(layout.focused_zone < layout.zones.len());
    }

    // --- Focus direction tests ---

    #[test]
    fn test_focus_direction_left_right() {
        // AC-ZT-05: 2-col, focus right then left.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0
        layout.add_window(wid(2)); // zone 1

        // Focus is at zone 1 (last added). Move left.
        layout.focused_zone = 0;
        assert_eq!(layout.focused(), Some(wid(1)));

        let result = layout.focus_direction(Direction::Right);
        assert_eq!(result, Some(wid(2)));
        assert_eq!(layout.focused_zone, 1);

        let result = layout.focus_direction(Direction::Left);
        assert_eq!(result, Some(wid(1)));
        assert_eq!(layout.focused_zone, 0);
    }

    #[test]
    fn test_focus_direction_boundary_noop() {
        // AC-ZT-06: rightmost zone, focus right = no-op.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1));
        layout.add_window(wid(2));

        layout.focused_zone = 1; // rightmost
        let result = layout.focus_direction(Direction::Right);
        assert_eq!(result, Some(wid(2))); // unchanged
        assert_eq!(layout.focused_zone, 1);

        layout.focused_zone = 0; // leftmost
        let result = layout.focus_direction(Direction::Left);
        assert_eq!(result, Some(wid(1))); // unchanged
        assert_eq!(layout.focused_zone, 0);
    }

    #[test]
    fn test_focus_direction_up_down_composite() {
        // Main-stack: focus from top-right to bottom-right.
        let (root, fill_order, primary) = composite_main_stack();
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0 (left)
        layout.add_window(wid(2)); // zone 1 (top-right)
        layout.add_window(wid(3)); // zone 2 (bottom-right)

        // Focus on zone 1 (top-right).
        layout.focused_zone = 1;
        let result = layout.focus_direction(Direction::Down);
        assert_eq!(result, Some(wid(3)));
        assert_eq!(layout.focused_zone, 2);

        let result = layout.focus_direction(Direction::Up);
        assert_eq!(result, Some(wid(2)));
        assert_eq!(layout.focused_zone, 1);
    }

    // --- Move to zone tests ---

    #[test]
    fn test_move_to_zone_swap() {
        // AC-ZT-08: 2-col, move window right swaps with destination.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0
        layout.add_window(wid(2)); // zone 1

        layout.focused_zone = 0;
        let moved = layout.move_to_zone(Direction::Right);
        assert!(moved);
        assert_eq!(layout.focused_zone, 1);

        // Windows should be swapped.
        assert_eq!(layout.zones[0], vec![wid(2)]);
        assert_eq!(layout.zones[1], vec![wid(1)]);
    }

    #[test]
    fn test_move_to_zone_boundary_noop() {
        // AC-ZT-09: rightmost zone, move right = no-op.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1));
        layout.add_window(wid(2));

        layout.focused_zone = 1;
        let moved = layout.move_to_zone(Direction::Right);
        assert!(!moved);
        assert_eq!(layout.focused_zone, 1);

        // Windows unchanged.
        assert_eq!(layout.zones[0], vec![wid(1)]);
        assert_eq!(layout.zones[1], vec![wid(2)]);
    }

    // --- Promote tests ---

    #[test]
    fn test_promote_to_primary() {
        // AC-ZT-10: 3-col center-first, promote from left to center.
        let (root, fill_order, primary) = three_col([0.30, 0.40, 0.30]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // center (zone 1, primary)
        layout.add_window(wid(2)); // left (zone 0)
        layout.add_window(wid(3)); // right (zone 2)

        // Focus on left zone.
        layout.focused_zone = 0;
        let promoted = layout.promote_to_primary();
        assert!(promoted);

        // wid(2) should now be in center (primary), wid(1) in left.
        assert_eq!(layout.zones[1], vec![wid(2)]); // center
        assert_eq!(layout.zones[0], vec![wid(1)]); // left
        assert_eq!(layout.focused_zone, 1); // focus follows to primary
    }

    #[test]
    fn test_promote_already_primary_noop() {
        // AC-ZT-11: already in primary = no-op.
        let (root, fill_order, primary) = three_col([0.30, 0.40, 0.30]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // center (zone 1, primary)

        layout.focused_zone = 1; // already primary
        let promoted = layout.promote_to_primary();
        assert!(!promoted);
    }

    // --- Focus next/prev overflow cycling ---

    #[test]
    fn test_focus_next_prev_overflow() {
        // Zone with 2 stacked windows, cycling works.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0
        layout.add_window(wid(2)); // zone 1
        layout.add_window(wid(3)); // overflow zone 1

        layout.focused_zone = 1;
        assert_eq!(layout.focused(), Some(wid(2)));

        // focus_next rotates: [2, 3] -> [3, 2]
        let next = layout.focus_next();
        assert_eq!(next, Some(wid(3)));

        // focus_next again: [3, 2] -> [2, 3]
        let next = layout.focus_next();
        assert_eq!(next, Some(wid(2)));

        // focus_prev: [2, 3] -> [3, 2]
        let prev = layout.focus_prev();
        assert_eq!(prev, Some(wid(3)));
    }

    // --- All windows visible ---

    #[test]
    fn test_all_windows_visible() {
        // AC-ZT-17: 2 windows in 2 zones, both get on-screen rects.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0
        layout.add_window(wid(2)); // zone 1

        let positions = layout.calculate_positions(screen(), gaps(), false);
        assert_eq!(positions.len(), 2);

        // Both windows should be on-screen (x < 10000).
        for (_, rect) in &positions {
            assert!(rect.x < 5000.0, "window should be on-screen, got x={}", rect.x);
            assert!(rect.y < 5000.0, "window should be on-screen, got y={}", rect.y);
        }
    }

    // --- Ratio normalization ---

    #[test]
    fn test_ratio_normalization() {
        // Ratios [0.30, 0.30, 0.30] should normalize to sum 1.0.
        let result = normalize_ratios(&[0.30, 0.30, 0.30]);
        let sum: f64 = result.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);
        // Each should be 1/3.
        for r in &result {
            assert!((r - 1.0 / 3.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_ratio_normalization_already_valid() {
        let result = normalize_ratios(&[0.50, 0.50]);
        assert_eq!(result, vec![0.50, 0.50]);
    }

    // --- C1: Per-element ratio validation ---

    #[test]
    fn test_ratio_zero_falls_back_to_equal() {
        // C1: [0.0, 1.0] should produce equal-weight fallback.
        let result = normalize_ratios(&[0.0, 1.0]);
        assert_eq!(result, vec![0.5, 0.5]);
    }

    #[test]
    fn test_ratio_negative_falls_back_to_equal() {
        let result = normalize_ratios(&[-0.5, 1.5]);
        assert_eq!(result, vec![0.5, 0.5]);
    }

    #[test]
    fn test_ratio_nan_falls_back_to_equal() {
        let result = normalize_ratios(&[f64::NAN, 0.5]);
        assert_eq!(result, vec![0.5, 0.5]);
    }

    // --- C2: fill_order bounds assert ---

    #[test]
    #[should_panic(expected = "fill_order contains out-of-bounds index")]
    #[cfg(debug_assertions)]
    fn test_fill_order_bounds_assert() {
        // C2: debug_assert on fill_order with out-of-bounds index.
        let root = ZoneNode::HSplit {
            ratios: vec![0.50, 0.50],
            children: vec![ZoneNode::Leaf, ZoneNode::Leaf],
        };
        // zone_count = 2, but fill_order references index 5.
        let _layout = ZoneLayout::new(root, vec![0, 5], 0);
    }

    // --- A1: Remove window from non-focused zone ---

    #[test]
    fn test_remove_window_from_non_focused_zone_preserves_focus() {
        // A1: removing a window from a non-focused zone should not change focus.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0
        layout.add_window(wid(2)); // zone 1

        // Focus zone 0.
        layout.focused_zone = 0;
        assert_eq!(layout.focused(), Some(wid(1)));

        // Remove from zone 1 (non-focused).
        layout.remove_window(wid(2));

        // Focus should still be on zone 0, window 1.
        assert_eq!(layout.focused(), Some(wid(1)));
        assert_eq!(layout.focused_zone, 0);
    }

    // --- Direction serde ---

    #[test]
    fn test_2row_fill_order() {
        // 2-row uses top-to-bottom fill order.
        let (root, fill_order, primary) = two_row([0.60, 0.40]);
        assert_eq!(fill_order, vec![0, 1]);
        assert_eq!(primary, 0);

        let mut layout = ZoneLayout::new(root, fill_order, primary);
        layout.add_window(wid(1)); // zone 0 (top)
        layout.add_window(wid(2)); // zone 1 (bottom)

        assert_eq!(layout.zones[0], vec![wid(1)]);
        assert_eq!(layout.zones[1], vec![wid(2)]);
    }

    #[test]
    fn test_direction_serde_roundtrip() {
        let dirs = [
            Direction::Left,
            Direction::Right,
            Direction::Up,
            Direction::Down,
        ];
        for dir in &dirs {
            let json = serde_json::to_string(dir).unwrap();
            let back: Direction = serde_json::from_str(&json).unwrap();
            assert_eq!(*dir, back);
        }
    }

    #[test]
    fn test_direction_serde_snake_case() {
        let json = serde_json::to_string(&Direction::Left).unwrap();
        assert_eq!(json, "\"left\"");
    }

    // --- Bug fix: add_window fills empty zones after remove ---

    #[test]
    fn test_add_window_fills_hole_after_remove() {
        // Bug 1: add 2 windows to 2-zone layout, remove from zone 0,
        // add new window — it should go to the now-empty zone 0.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0
        layout.add_window(wid(2)); // zone 1

        assert_eq!(layout.zones[0], vec![wid(1)]);
        assert_eq!(layout.zones[1], vec![wid(2)]);

        // Remove window from zone 0.
        layout.remove_window(wid(1));
        assert!(layout.zones[0].is_empty());

        // Add a new window — should fill the empty zone 0, not overflow to zone 1.
        layout.add_window(wid(3));
        assert_eq!(layout.zones[0], vec![wid(3)]);
        assert_eq!(layout.zones[1], vec![wid(2)]);
    }

    #[test]
    fn test_add_window_fills_hole_3col_center_first() {
        // 3-col center-first: remove center window, add new one → fills center.
        let (root, fill_order, primary) = three_col([0.30, 0.40, 0.30]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // center (zone 1)
        layout.add_window(wid(2)); // left (zone 0)
        layout.add_window(wid(3)); // right (zone 2)

        layout.remove_window(wid(1)); // center now empty
        assert!(layout.zones[1].is_empty());

        layout.add_window(wid(4)); // should fill center (first empty in fill_order)
        assert_eq!(layout.zones[1], vec![wid(4)]);
        assert_eq!(layout.zones[0], vec![wid(2)]);
        assert_eq!(layout.zones[2], vec![wid(3)]);
    }

    // --- Bug fix: focus_direction cycles overflow at boundary ---

    #[test]
    fn test_focus_direction_cycles_overflow_at_boundary() {
        // Bug 2: 2-col layout with 3 windows (zone 1 has 2 stacked),
        // pressing Down when no zone below should cycle to next overflow window.
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0
        layout.add_window(wid(2)); // zone 1
        layout.add_window(wid(3)); // overflow zone 1

        layout.focused_zone = 1;
        assert_eq!(layout.focused(), Some(wid(2)));

        // Down with no zone below in a 2-col (horizontal) layout → cycles overflow.
        let result = layout.focus_direction(Direction::Down);
        assert_eq!(result, Some(wid(3))); // cycled to overflow window

        // Down again → cycles back.
        let result = layout.focus_direction(Direction::Down);
        assert_eq!(result, Some(wid(2)));
    }

    #[test]
    fn test_focus_direction_up_cycles_overflow_backward() {
        // Up with no zone above → cycles overflow backward (focus_prev).
        let (root, fill_order, primary) = two_col([0.50, 0.50]);
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0
        layout.add_window(wid(2)); // zone 1
        layout.add_window(wid(3)); // overflow zone 1

        layout.focused_zone = 1;
        assert_eq!(layout.focused(), Some(wid(2)));

        // Up with no zone above → focus_prev, rotates [2,3] → [3,2].
        let result = layout.focus_direction(Direction::Up);
        assert_eq!(result, Some(wid(3)));
    }

    // --- Bug fix: CLI move-to-zone up/down ---

    #[test]
    fn test_move_to_zone_up_down_composite() {
        // move_to_zone with Up/Down in composite layout.
        let (root, fill_order, primary) = composite_main_stack();
        let mut layout = ZoneLayout::new(root, fill_order, primary);

        layout.add_window(wid(1)); // zone 0 (left)
        layout.add_window(wid(2)); // zone 1 (top-right)
        layout.add_window(wid(3)); // zone 2 (bottom-right)

        // Focus top-right, move down.
        layout.focused_zone = 1;
        let moved = layout.move_to_zone(Direction::Down);
        assert!(moved);
        assert_eq!(layout.focused_zone, 2);
        assert_eq!(layout.zones[2][0], wid(2));
        assert_eq!(layout.zones[1][0], wid(3));
    }
}
