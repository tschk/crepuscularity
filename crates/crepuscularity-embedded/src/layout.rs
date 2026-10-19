//! Flexbox-style layout for [`EmbeddedDocument`] trees.

#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};

use crate::document::{Align, EmbeddedDocument, EmbeddedNode, FlexDir, Justify, Rect, SizeHint};
use crate::font::FontMetrics;
use crate::screen::ScreenSize;

pub fn layout_tree(root: &mut EmbeddedNode, screen: ScreenSize) {
    measure_intrinsic(root);
    root.bounds = if root.style.hidden {
        Rect::default()
    } else {
        Rect::new(0, 0, screen.width, screen.height)
    };
    if !root.style.hidden {
        let available = root.bounds;
        if root.style.width != SizeHint::Auto {
            root.bounds.w = cross_size(root, false, available);
        }
        if root.style.height != SizeHint::Auto {
            root.bounds.h = cross_size(root, true, available);
        }
        layout_children(root);
    }
}

pub fn layout_document(doc: &mut EmbeddedDocument) {
    for node in &mut doc.root {
        layout_tree(node, doc.screen);
    }
    doc.reindex_by_id();
}

fn measure_intrinsic(node: &mut EmbeddedNode) {
    if node.style.hidden {
        node.bounds = Rect::default();
        return;
    }
    let (width, height) = if let Some(text) = &node.text {
        FontMetrics::default().measure_text(text)
    } else {
        let row = node.style.flex_dir == FlexDir::Row;
        let (mut width, mut height, mut count) = (0u16, 0u16, 0u16);
        for child in &mut node.children {
            measure_intrinsic(child);
            if child.style.hidden {
                continue;
            }
            if row {
                width = width.saturating_add(child.bounds.w);
                height = height.max(child.bounds.h);
            } else {
                width = width.max(child.bounds.w);
                height = height.saturating_add(child.bounds.h);
            }
            count = count.saturating_add(1);
        }
        let gaps = node.style.gap.saturating_mul(count.saturating_sub(1));
        if row {
            width = width.saturating_add(gaps);
        } else {
            height = height.saturating_add(gaps);
        }
        (width.max(32), height.max(16))
    };
    let intrinsic = |hint, size| match hint {
        SizeHint::Auto => size,
        SizeHint::Fixed(value) => value,
        // Relative and flexible sizes resolve within the eventual allocation.
        _ => 0,
    };
    // Bounds hold each intrinsic size until its parent assigns the final rectangle.
    node.bounds = Rect::new(
        0,
        0,
        intrinsic(
            node.style.width,
            width.saturating_add(node.style.horizontal_inset()),
        ),
        intrinsic(
            node.style.height,
            height.saturating_add(node.style.vertical_inset()),
        ),
    );
}

fn layout_children(parent: &mut EmbeddedNode) {
    if let Some(text) = &parent.text {
        let (tw, th) = FontMetrics::default().measure_text(text);
        if parent.style.width == SizeHint::Auto {
            parent.bounds.w = tw
                .saturating_add(parent.style.horizontal_inset())
                .min(parent.bounds.w);
        }
        if parent.style.height == SizeHint::Auto {
            parent.bounds.h = th
                .saturating_add(parent.style.vertical_inset())
                .min(parent.bounds.h);
        }
        return;
    }
    if parent.children.is_empty() {
        return;
    }
    let inner = parent.style.content_bounds(parent.bounds);
    let gap = parent.style.gap;
    let row = parent.style.flex_dir == FlexDir::Row;

    for child in &mut parent.children {
        if child.style.hidden {
            child.bounds = Rect::default();
        }
    }
    let visible: Vec<_> = parent
        .children
        .iter()
        .filter(|child| !child.style.hidden)
        .collect();
    let main_sizes = compute_main_sizes(&visible, row, gap, inner);
    let offsets = compute_offsets(&main_sizes, parent.style.justify, gap, inner, row);

    for (i, child) in parent
        .children
        .iter_mut()
        .filter(|child| !child.style.hidden)
        .enumerate()
    {
        let cross_hint = if row {
            child.style.height
        } else {
            child.style.width
        };
        let cross = if parent.style.align == Align::Stretch && cross_hint == SizeHint::Auto {
            if row {
                inner.h
            } else {
                inner.w
            }
        } else {
            cross_size(child, row, inner)
        };
        let (x, y, w, h) = if row {
            (
                inner.x.saturating_add(offsets[i]),
                align_cross(child, inner.y, cross, inner.h, parent.style.align),
                main_sizes[i],
                cross,
            )
        } else {
            (
                align_cross(child, inner.x, cross, inner.w, parent.style.align),
                inner.y.saturating_add(offsets[i]),
                cross,
                main_sizes[i],
            )
        };
        child.bounds = Rect::new(x, y, w, h);
        if child.text.is_none() {
            layout_children(child);
        }
    }
}

fn compute_main_sizes(children: &[&EmbeddedNode], row: bool, gap: u16, inner: Rect) -> Vec<u16> {
    let n = children.len();
    let mut main_sizes: Vec<u16> = children
        .iter()
        .map(|c| desired_main(c, row, inner))
        .collect();

    let mut flex_count = 0usize;
    let mut fixed_sum = 0u16;
    for (i, child) in children.iter().enumerate() {
        let hint = main_hint(child, row);
        if hint == SizeHint::Flex1 || hint == SizeHint::Fill {
            flex_count += 1;
        } else {
            fixed_sum = fixed_sum.saturating_add(main_sizes[i]);
        }
    }
    let gaps = gap.saturating_mul(n.saturating_sub(1).min(u16::MAX as usize) as u16);
    let avail_main = inner_main(inner, row)
        .saturating_sub(fixed_sum)
        .saturating_sub(gaps);
    if let Some(each) = (avail_main as usize).checked_div(flex_count) {
        let mut remainder = avail_main as usize % flex_count;
        for (i, child) in children.iter().enumerate() {
            let hint = main_hint(child, row);
            if hint == SizeHint::Flex1 || hint == SizeHint::Fill {
                main_sizes[i] = (each + usize::from(remainder > 0)) as u16;
                remainder = remainder.saturating_sub(1);
            }
        }
    }

    main_sizes
}

fn compute_offsets(
    main_sizes: &[u16],
    justify: Justify,
    gap: u16,
    inner: Rect,
    row: bool,
) -> Vec<u16> {
    let n = main_sizes.len();
    let total_main: u16 = main_sizes
        .iter()
        .fold(0u16, |total, size| total.saturating_add(*size))
        .saturating_add(gap.saturating_mul(n.saturating_sub(1).min(u16::MAX as usize) as u16));
    let free_main = inner_main(inner, row).saturating_sub(total_main);

    let mut offsets: Vec<u16> = vec![0; n];
    let (mut pos, extra_gap) = match justify {
        Justify::Center if free_main > 0 => (free_main / 2, 0),
        Justify::Between if n > 1 && free_main > 0 => (0, (free_main as usize / (n - 1)) as u16),
        Justify::Around if n > 0 && free_main > 0 => {
            let space = (free_main as usize / n) as u16;
            (space / 2, space)
        }
        Justify::Evenly if n > 0 && free_main > 0 => {
            let space = (free_main as usize / n.saturating_add(1)) as u16;
            (space, space)
        }
        Justify::End if free_main > 0 => (free_main, 0),
        _ => (0, 0),
    };

    for (i, &size) in main_sizes.iter().enumerate() {
        offsets[i] = pos;
        pos = pos
            .saturating_add(size)
            .saturating_add(gap)
            .saturating_add(extra_gap);
    }

    offsets
}

fn inner_main(inner: Rect, row: bool) -> u16 {
    if row {
        inner.w
    } else {
        inner.h
    }
}

fn main_hint(child: &EmbeddedNode, row: bool) -> SizeHint {
    if row {
        child.style.width
    } else {
        child.style.height
    }
}

fn desired_main(child: &EmbeddedNode, row: bool, inner: Rect) -> u16 {
    if let Some(text) = child
        .text
        .as_ref()
        .filter(|_| main_hint(child, row) == SizeHint::Auto)
    {
        let metrics = FontMetrics::default();
        let (tw, th) = metrics.measure_text(text);
        let (size, pad) = if row {
            (tw, child.style.horizontal_inset())
        } else {
            (th, child.style.vertical_inset())
        };
        return size.saturating_add(pad).min(inner_main(inner, row));
    }
    let hint = main_hint(child, row);
    let avail = inner_main(inner, row);
    match hint {
        SizeHint::Fixed(v) => v.min(avail),
        SizeHint::Fill | SizeHint::Flex1 => avail,
        SizeHint::Fraction { num, den } => {
            if den == 0 {
                avail
            } else {
                ((avail as u32).saturating_mul(num as u32) / den as u32).min(u16::MAX as u32) as u16
            }
        }
        SizeHint::Auto => {
            if row {
                child.bounds.w.min(avail)
            } else {
                child.bounds.h.min(avail)
            }
        }
    }
}

fn cross_size(child: &EmbeddedNode, row: bool, inner: Rect) -> u16 {
    let hint = if row {
        child.style.height
    } else {
        child.style.width
    };
    let avail = if row { inner.h } else { inner.w };
    match hint {
        SizeHint::Fixed(v) => v.min(avail),
        SizeHint::Fill | SizeHint::Flex1 => avail,
        SizeHint::Fraction { num, den } => {
            if den == 0 {
                avail
            } else {
                ((avail as u32).saturating_mul(num as u32) / den as u32).min(u16::MAX as u32) as u16
            }
        }
        SizeHint::Auto => {
            if let Some(text) = &child.text {
                let metrics = FontMetrics::default();
                let (tw, th) = metrics.measure_text(text);
                let (size, pad) = if row {
                    (th, child.style.vertical_inset())
                } else {
                    (tw, child.style.horizontal_inset())
                };
                return size.saturating_add(pad).min(avail);
            }
            avail
        }
    }
}

fn align_cross(_child: &EmbeddedNode, start: u16, size: u16, avail: u16, align: Align) -> u16 {
    match align {
        Align::Center => start.saturating_add(avail.saturating_sub(size) / 2),
        Align::End => start.saturating_add(avail.saturating_sub(size)),
        _ => start,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::EmbeddedStyle;
    #[cfg(not(feature = "std"))]
    use alloc::{string::String, vec};
    #[cfg(feature = "std")]
    use std::{string::String, vec};

    #[test]
    fn row_text_uses_width_on_main_axis_and_height_on_cross_axis() {
        let label = EmbeddedNode {
            id: None,
            tag: String::from("span"),
            text: Some(String::from("abcd")),
            on_click: None,
            style: EmbeddedStyle {
                padding_x: 2,
                padding_y: 1,
                ..EmbeddedStyle::default()
            },
            bounds: Rect::default(),
            children: vec![],
        };
        let mut root = EmbeddedNode {
            id: None,
            tag: String::from("div"),
            text: None,
            on_click: None,
            style: EmbeddedStyle {
                flex_dir: FlexDir::Row,
                ..EmbeddedStyle::default()
            },
            bounds: Rect::default(),
            children: vec![label.clone(), label],
        };
        layout_tree(&mut root, ScreenSize::new(100, 100));
        assert_eq!(root.children[0].bounds, Rect::new(0, 0, 36, 10));
        assert_eq!(root.children[1].bounds, Rect::new(36, 0, 36, 10));
        let mut flex = root.children[0].clone();
        flex.text = None;
        flex.style.width = SizeHint::Flex1;
        assert_eq!(
            compute_main_sizes(&[&flex, &flex], true, 10, Rect::new(0, 0, 10, 10)),
            vec![0, 0]
        );
        root.children[0].id = Some(String::from("hidden"));
        root.children[0].style.hidden = true;
        root.children[0].style.bg = Some(crate::Color::rgb(255, 0, 0));
        root.children[1].id = Some(String::from("visible"));
        root.children[1].style.bg = Some(crate::Color::rgb(255, 255, 255));
        layout_tree(&mut root, ScreenSize::new(100, 100));
        assert_eq!(root.children[0].bounds, Rect::default());
        assert_eq!(root.children[1].bounds, Rect::new(0, 0, 36, 10));
        let doc = EmbeddedDocument::new(vec![root.clone()], ScreenSize::new(100, 100));
        assert_eq!(doc.hit_test(1, 1), Some("visible"));
        let mut fb = crate::Rgb565Buffer::new(doc.screen, crate::DEFAULT_BG);
        crate::paint::paint_document(&mut fb, &doc);
        assert_eq!(fb.pixels()[0], u16::MAX);
        root.style.hidden = true;
        let hidden = EmbeddedDocument::new(vec![root.clone()], doc.screen);
        assert_eq!(hidden.hit_test(1, 1), None);
        crate::paint::paint_document(&mut fb, &hidden);
        assert!(fb
            .pixels()
            .iter()
            .all(|pixel| *pixel == crate::DEFAULT_BG.to_rgb565().0));
        root.style.hidden = false;
        root.children[0].style.hidden = false;
        layout_tree(&mut root, doc.screen);
        assert_eq!(root.children[1].bounds.x, 36);
        root.children[0].style.invisible = true;
        layout_tree(&mut root, doc.screen);
        let invisible = EmbeddedDocument::new(vec![root.clone()], doc.screen);
        assert_eq!(invisible.root[0].children[1].bounds.x, 36);
        assert_eq!(invisible.hit_test(1, 1), None);
        crate::paint::paint_document(&mut fb, &invisible);
        assert_eq!(fb.pixels()[0], crate::DEFAULT_BG.to_rgb565().0);
        root.children[0].style.invisible = false;
        root.children[0].style.opacity_percent = Some(0);
        let transparent = EmbeddedDocument::new(vec![root.clone()], doc.screen);
        assert_eq!(transparent.hit_test(1, 1), Some("hidden"));
        crate::paint::paint_document(&mut fb, &transparent);
        assert_eq!(fb.pixels()[0], crate::DEFAULT_BG.to_rgb565().0);
        root.children[0].style.opacity_percent = None;
        root.style.padding_x = u16::MAX;
        root.style.padding_y = u16::MAX;
        layout_tree(&mut root, ScreenSize::new(100, 100));
        assert_eq!(root.children[0].bounds.w, 0);
        assert_eq!(root.children[0].bounds.h, 0);
        root.text = Some(String::from("text"));
        layout_tree(&mut root, ScreenSize::new(100, 100));
        assert_eq!(root.bounds.w, 100);
        assert_eq!(root.bounds.h, 100);
    }

    #[test]
    fn around_and_evenly_distribute_remaining_space() {
        let inner = Rect::new(0, 0, 100, 1);
        assert_eq!(
            compute_offsets(&[10, 10], Justify::Around, 0, inner, true),
            vec![20, 70]
        );
        assert_eq!(
            compute_offsets(&[10, 10], Justify::Evenly, 0, inner, true),
            vec![26, 62]
        );
        for (justify, expected) in [
            (Justify::Around, vec![17, 72]),
            (Justify::Evenly, vec![23, 66]),
        ] {
            assert_eq!(
                compute_offsets(&[10, 10], justify, 10, inner, true),
                expected
            );
            assert_eq!(
                compute_offsets(&[10, 10], justify, 10, Rect::new(0, 0, 1, 100), false),
                expected
            );
        }
        for justify in [Justify::Around, Justify::Evenly] {
            assert_eq!(compute_offsets(&[10], justify, 0, inner, true), vec![45]);
            assert!(compute_offsets(&[], justify, 0, inner, true).is_empty());
            assert_eq!(
                compute_offsets(&[60, 60], justify, 0, inner, true),
                vec![0, 60]
            );
        }
    }

    #[test]
    fn overflowing_child_sizes_do_not_wrap_justification() {
        assert_eq!(
            compute_offsets(
                &[40000, 40000],
                Justify::End,
                10,
                Rect::new(0, 0, u16::MAX, 1),
                true
            ),
            vec![0, 40010],
        );
        let sizes = vec![0; u16::MAX as usize + 2];
        let offsets = compute_offsets(&sizes, Justify::Between, 0, Rect::new(0, 0, 10, 1), true);
        assert!(offsets.iter().all(|offset| *offset == 0));
    }

    #[test]
    fn test_layout_tree_sets_bounds() {
        let mut root = EmbeddedNode {
            id: None,
            tag: String::from("div"),
            text: None,
            on_click: None,
            style: EmbeddedStyle::default(),
            bounds: Rect::new(0, 0, 0, 0),
            children: vec![],
        };
        let screen = ScreenSize::new(800, 600);
        layout_tree(&mut root, screen);

        assert_eq!(root.bounds, Rect::new(0, 0, 800, 600));

        let mut leaf = root.clone();
        leaf.style.height = SizeHint::Fixed(8);
        let mut wrapper = root.clone();
        wrapper.children = vec![leaf.clone(), leaf.clone(), leaf.clone()];
        root.children = vec![wrapper, leaf];
        layout_tree(&mut root, screen);
        assert_eq!(root.children[1].bounds.y, 24);
        for child in &mut root.children[0].children {
            child.style.height = SizeHint::Fixed(4);
        }
        layout_tree(&mut root, screen);
        assert_eq!(root.children[1].bounds.y, 16);
    }

    #[test]
    fn test_layout_document_layouts_all_roots_and_reindexes() {
        let node1 = EmbeddedNode {
            id: Some(String::from("node1")),
            tag: String::from("div"),
            text: None,
            on_click: None,
            style: EmbeddedStyle::default(),
            bounds: Rect::new(0, 0, 0, 0),
            children: vec![],
        };
        let node2 = EmbeddedNode {
            id: Some(String::from("node2")),
            tag: String::from("div"),
            text: None,
            on_click: None,
            style: EmbeddedStyle::default(),
            bounds: Rect::new(0, 0, 0, 0),
            children: vec![],
        };
        let mut doc = EmbeddedDocument::new(vec![node1, node2], ScreenSize::new(1024, 768));

        doc.screen = ScreenSize::new(1920, 1080);
        layout_document(&mut doc);

        assert_eq!(doc.root[0].bounds, Rect::new(0, 0, 1920, 1080));
        assert_eq!(doc.root[1].bounds, Rect::new(0, 0, 1920, 1080));

        assert!(doc.by_id.contains_key("node1"));
        assert!(doc.by_id.contains_key("node2"));
    }
}
