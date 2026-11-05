//! Layout tree and hit-testing for embedded UI.

use crate::color::{Color, Rgb888};
use crate::screen::ScreenSize;

#[cfg(not(feature = "std"))]
use alloc::{collections::BTreeMap, string::String, vec, vec::Vec};
#[cfg(feature = "std")]
use std::{collections::BTreeMap, string::String, vec::Vec};

pub const DEFAULT_BG: Color = Color::Rgb888(Rgb888::new(9, 9, 11));
pub const DEFAULT_TEXT: Color = Color::Rgb888(Rgb888::new(244, 244, 245));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    pub const fn new(x: u16, y: u16, w: u16, h: u16) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, px: u16, py: u16) -> bool {
        px >= self.x && py >= self.y && px - self.x < self.w && py - self.y < self.h
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SizeHint {
    #[default]
    Auto,
    Fill,
    Fixed(u16),
    Flex1,
    /// `w-1/2`, `h-1/3`, etc.
    Fraction {
        num: u16,
        den: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlexDir {
    #[default]
    Column,
    Row,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Justify {
    #[default]
    Start,
    Center,
    Between,
    End,
    Around,
    Evenly,
}

#[derive(Debug, Clone, Default)]
pub struct EmbeddedStyle {
    pub bg: Option<Color>,
    pub text: Option<Color>,
    pub padding: u16,
    pub padding_x: u16,
    pub padding_y: u16,
    pub padding_t: u16,
    pub padding_b: u16,
    pub padding_l: u16,
    pub padding_r: u16,
    /// Parsed spacing metadata; margins are not yet applied by layout.
    pub margin: u16,
    pub margin_x: u16,
    pub margin_y: u16,
    pub margin_t: u16,
    pub margin_b: u16,
    pub margin_l: u16,
    pub margin_r: u16,
    pub gap: u16,
    pub flex_dir: FlexDir,
    pub align: Align,
    pub justify: Justify,
    pub border_width: u16,
    pub border_color: Option<Color>,
    pub width: SizeHint,
    pub height: SizeHint,
    /// 0–100 when set via `opacity-*`; fractional opacity currently paints opaque.
    pub opacity_percent: Option<u8>,
    pub hidden: bool,
    pub invisible: bool,
    /// Parsed font size; the renderer currently uses a fixed 8×8 bitmap font.
    pub font_size_px: u16,
    /// Parsed font weight; the renderer currently uses the same glyphs for all weights.
    pub font_bold: bool,
}

macro_rules! pad_margin_getter {
    ($name:ident, $specific:ident, $axis:ident, $shorthand:ident) => {
        pub fn $name(&self) -> u16 {
            if self.$specific > 0 {
                self.$specific
            } else {
                Self::axis_or_shorthand(self.$axis, self.$shorthand)
            }
        }
    };
}

impl EmbeddedStyle {
    fn axis_or_shorthand(axis: u16, shorthand: u16) -> u16 {
        if axis > 0 {
            axis
        } else {
            shorthand
        }
    }

    pad_margin_getter!(pad_left, padding_l, padding_x, padding);
    pad_margin_getter!(pad_right, padding_r, padding_x, padding);
    pad_margin_getter!(pad_top, padding_t, padding_y, padding);
    pad_margin_getter!(pad_bottom, padding_b, padding_y, padding);

    pub(crate) fn horizontal_inset(&self) -> u16 {
        self.pad_left()
            .saturating_add(self.pad_right())
            .saturating_add(self.border_width.saturating_mul(2))
    }

    pub(crate) fn vertical_inset(&self) -> u16 {
        self.pad_top()
            .saturating_add(self.pad_bottom())
            .saturating_add(self.border_width.saturating_mul(2))
    }

    pub(crate) fn content_bounds(&self, bounds: Rect) -> Rect {
        Rect::new(
            bounds
                .x
                .saturating_add(self.border_width)
                .saturating_add(self.pad_left()),
            bounds
                .y
                .saturating_add(self.border_width)
                .saturating_add(self.pad_top()),
            bounds.w.saturating_sub(self.horizontal_inset()),
            bounds.h.saturating_sub(self.vertical_inset()),
        )
    }

    pub fn pad_x(&self) -> u16 {
        self.pad_left().max(self.pad_right())
    }

    pub fn pad_y(&self) -> u16 {
        self.pad_top().max(self.pad_bottom())
    }

    pad_margin_getter!(margin_left, margin_l, margin_x, margin);
    pad_margin_getter!(margin_right, margin_r, margin_x, margin);
    pad_margin_getter!(margin_top, margin_t, margin_y, margin);
    pad_margin_getter!(margin_bottom, margin_b, margin_y, margin);

    pub fn opacity_fraction(&self) -> u8 {
        match self.opacity_percent {
            None => 255,
            Some(p) => ((p as u32 * 255) / 100).min(255) as u8,
        }
    }

    pub fn font_size_px(&self) -> u16 {
        if self.font_size_px == 0 {
            8
        } else {
            self.font_size_px.clamp(6, 32)
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmbeddedNode {
    pub id: Option<String>,
    pub tag: String,
    pub text: Option<String>,
    pub on_click: Option<String>,
    pub style: EmbeddedStyle,
    pub bounds: Rect,
    pub children: Vec<EmbeddedNode>,
}

#[derive(Debug, Clone)]
pub struct EmbeddedDocument {
    pub root: Vec<EmbeddedNode>,
    pub screen: ScreenSize,
    pub by_id: BTreeMap<String, Vec<usize>>,
}

impl EmbeddedDocument {
    pub fn new(root: Vec<EmbeddedNode>, screen: ScreenSize) -> Self {
        let mut by_id = BTreeMap::new();
        for (i, node) in root.iter().enumerate() {
            let mut path = vec![i];
            index_node(node, &mut by_id, &mut path);
        }
        Self {
            root,
            screen,
            by_id,
        }
    }

    pub fn reindex_by_id(&mut self) {
        self.by_id.clear();
        for (i, node) in self.root.iter().enumerate() {
            let mut path = vec![i];
            index_node(node, &mut self.by_id, &mut path);
        }
    }

    pub fn hit_test(&self, x: u16, y: u16) -> Option<&str> {
        self.node_at(x, y)
    }

    pub fn node_by_id(&self, id: &str) -> Option<&EmbeddedNode> {
        let path = self.by_id.get(id)?;
        resolve_path(&self.root, path)
    }

    pub fn node_at(&self, x: u16, y: u16) -> Option<&str> {
        if x >= self.screen.width || y >= self.screen.height {
            return None;
        }
        let mut best: Option<&EmbeddedNode> = None;
        for node in &self.root {
            if let Some(hit) = deepest_hit(node, x, y) {
                if hit.id.is_some() {
                    best = Some(hit);
                }
            }
        }
        best.and_then(|n| n.id.as_deref())
    }
}

fn index_node(node: &EmbeddedNode, map: &mut BTreeMap<String, Vec<usize>>, path: &mut Vec<usize>) {
    if let Some(id) = &node.id {
        map.insert(id.clone(), path.clone());
    }
    if node.children.is_empty() {
        return;
    }
    let old_len = path.len();
    path.push(0);
    for (i, child) in node.children.iter().enumerate() {
        path[old_len] = i;
        index_node(child, map, path);
    }
    path.truncate(old_len);
}

fn resolve_path<'a>(roots: &'a [EmbeddedNode], path: &[usize]) -> Option<&'a EmbeddedNode> {
    let mut node = roots.get(*path.first()?)?;
    for &idx in &path[1..] {
        node = node.children.get(idx)?;
    }
    Some(node)
}

fn deepest_hit(node: &EmbeddedNode, x: u16, y: u16) -> Option<&EmbeddedNode> {
    if node.style.hidden || node.style.invisible {
        return None;
    }
    // Painting permits visible overflow, so ancestor bounds cannot prune children.
    let mut best = node.bounds.contains(x, y).then_some(node);
    for child in &node.children {
        if let Some(hit) = deepest_hit(child, x, y) {
            best = Some(if hit.id.is_some() { hit } else { node });
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_contains_uses_half_open_bounds_without_coordinate_overflow() {
        let rect = Rect::new(u16::MAX, u16::MAX, 1, 1);
        assert!(rect.contains(u16::MAX, u16::MAX));
        assert!(!rect.contains(u16::MAX - 1, u16::MAX));
        assert!(!rect.contains(u16::MAX, u16::MAX - 1));
        assert!(!Rect::new(u16::MAX, u16::MAX, 0, 1).contains(u16::MAX, u16::MAX));
        let rect = Rect::new(10, 20, 2, 3);
        assert!(rect.contains(11, 22));
        assert!(!rect.contains(12, 22));
        assert!(!rect.contains(11, 23));
    }

    #[test]
    fn node_at_prefers_deepest_id() {
        let child = EmbeddedNode {
            id: Some("child".into()),
            tag: "div".into(),
            text: None,
            on_click: None,
            style: EmbeddedStyle::default(),
            bounds: Rect::new(10, 10, 20, 20),
            children: vec![],
        };
        let root = EmbeddedNode {
            id: Some("root".into()),
            tag: "div".into(),
            text: None,
            on_click: None,
            style: EmbeddedStyle::default(),
            bounds: Rect::new(0, 0, 100, 100),
            children: vec![child],
        };
        let doc = EmbeddedDocument::new(vec![root], ScreenSize::new(100, 100));
        assert_eq!(doc.node_at(15, 15), Some("child"));
        let mut unnamed = doc.root[0].clone();
        unnamed.children[0].id = None;
        let doc = EmbeddedDocument::new(vec![unnamed], ScreenSize::new(100, 100));
        assert_eq!(doc.node_at(15, 15), Some("root"));
        let mut overlap = doc.root[0].clone();
        overlap.children[0].id = Some("back".into());
        let mut front = overlap.children[0].clone();
        front.id = Some("front".into());
        overlap.children.push(front);
        let doc = EmbeddedDocument::new(vec![overlap], ScreenSize::new(100, 100));
        assert_eq!(doc.node_at(15, 15), Some("front"));
        let mut overflow = doc.clone();
        overflow.screen = ScreenSize::new(200, 200);
        for child in &mut overflow.root[0].children {
            child.bounds = Rect::new(110, 110, 20, 20);
        }
        assert_eq!(overflow.node_at(115, 115), Some("front"));
        overflow.root[0].children[1].id = None;
        assert_eq!(overflow.node_at(115, 115), Some("root"));
        assert_eq!(overflow.node_at(150, 150), None);
        let mut clipped = doc.clone();
        clipped.screen = ScreenSize::new(20, 20);
        assert_eq!(clipped.node_at(19, 19), Some("front"));
        for (x, y) in [(20, 19), (19, 20), (99, 99), (u16::MAX, u16::MAX)] {
            assert_eq!(clipped.node_at(x, y), None);
            assert_eq!(clipped.hit_test(x, y), None);
        }
        clipped.screen = ScreenSize::new(0, 0);
        assert_eq!(clipped.node_at(0, 0), None);
    }
}
