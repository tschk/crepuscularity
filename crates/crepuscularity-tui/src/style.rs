//! Tailwind-class → ratatui style / layout hints.
//!
//! A small subset of Tailwind utilities that map cleanly onto terminal concepts:
//! colours, text modifiers, borders, flex direction, and dimensional constraints.

use ratatui::layout::{Alignment, Constraint, Direction};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{BorderType, Borders, Padding};

/// How an element wants to be sized along one axis.
#[derive(Debug, Clone, Default)]
pub enum SizeHint {
    /// Take an equal share of whatever space remains after `Length`/`Percentage` items
    /// are satisfied (`Constraint::Fill(1)`).
    #[default]
    Fill,
    /// Exact terminal columns / rows.
    Fixed(u16),
    /// Percentage of the parent area.
    Percentage(u16),
    /// Fit to content — size to the natural height/width of children.
    /// Resolved during the build phase, not from a static class.
    Fit,
}

impl SizeHint {
    pub fn to_constraint(&self) -> Constraint {
        match self {
            SizeHint::Fill => Constraint::Fill(1),
            SizeHint::Fixed(n) => Constraint::Length(*n),
            SizeHint::Percentage(p) => Constraint::Percentage(*p),
            SizeHint::Fit => Constraint::Min(0),
        }
    }
}

/// All style/layout information extracted from a set of Tailwind-like class names.
#[derive(Debug, Clone)]
pub struct StyleHints {
    // ── Colours ──────────────────────────────────────────────────────────────
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    // ── Text modifiers ───────────────────────────────────────────────────────
    pub modifiers: Modifier,
    // ── Borders ──────────────────────────────────────────────────────────────
    pub borders: Borders,
    pub border_type: BorderType,
    pub border_fg: Option<Color>,
    pub focus_border_fg: Option<Color>,
    // ── Element id (from `id="..."` or `#id`) ────────────────────────────────
    pub id: Option<String>,
    pub focusable: bool,
    // ── Title (from `title="..."` binding, not a class) ──────────────────────
    pub title: Option<String>,
    pub title_alignment: Alignment,
    // ── Inner padding ────────────────────────────────────────────────────────
    pub padding: Padding,
    // ── Text alignment ───────────────────────────────────────────────────────
    pub alignment: Alignment,
    // ── Flex layout ──────────────────────────────────────────────────────────
    /// Direction children are stacked. `flex-col` (default) → `Vertical`;
    /// `flex` / `flex-row` → `Horizontal`.
    pub direction: Direction,
    /// Gap (spacing) between children in terminal cells.
    pub gap: u16,
    // ── Dimensional hints for constraint calculation ──────────────────────────
    pub width: SizeHint,
    pub height: SizeHint,
}

impl Default for StyleHints {
    fn default() -> Self {
        StyleHints {
            fg: None,
            bg: None,
            modifiers: Modifier::empty(),
            borders: Borders::NONE,
            border_type: BorderType::Plain,
            border_fg: None,
            focus_border_fg: None,
            id: None,
            focusable: false,
            title: None,
            title_alignment: Alignment::Left,
            padding: Padding::ZERO,
            alignment: Alignment::Left,
            direction: Direction::Vertical,
            gap: 0,
            width: SizeHint::Fill,
            height: SizeHint::Fill,
        }
    }
}

impl StyleHints {
    /// Return a `ratatui::Style` for text / foreground colouring.
    pub fn to_style(&self) -> Style {
        let mut s = Style::default().add_modifier(self.modifiers);
        if let Some(c) = self.fg {
            s = s.fg(c);
        }
        if let Some(c) = self.bg {
            s = s.bg(c);
        }
        s
    }

    /// Whether a `Block` widget is needed (borders, padding, or title).
    pub fn needs_block(&self) -> bool {
        self.borders != Borders::NONE || self.title.is_some() || self.padding != Padding::ZERO
    }

    /// Constraint this element wants when inside a parent laid out along `parent_dir`.
    pub fn constraint_for(&self, parent_dir: Direction) -> Constraint {
        match parent_dir {
            Direction::Horizontal => self.width.to_constraint(),
            Direction::Vertical => self.height.to_constraint(),
        }
    }
}

// ─── Public entry point ───────────────────────────────────────────────────────

/// Parse a slice of Tailwind-like class strings into [`StyleHints`].
pub fn parse_classes(classes: &[String]) -> StyleHints {
    let mut hints = StyleHints::default();
    for class in classes {
        apply_class(class.as_str(), &mut hints);
    }
    hints
}

// ─── Class → hints ────────────────────────────────────────────────────────────

fn apply_class(class: &str, h: &mut StyleHints) {
    if apply_layout(class, h)
        || apply_modifiers(class, h)
        || apply_borders(class, h)
        || apply_alignment(class, h)
        || apply_named_colors(class, h)
    {
        return;
    }
    apply_parametric(class, h);
}

fn apply_layout(class: &str, h: &mut StyleHints) -> bool {
    match class {
        "flex-col" | "block" => h.direction = Direction::Vertical,
        "flex" | "flex-row" | "inline-flex" => h.direction = Direction::Horizontal,
        "w-full" => h.width = SizeHint::Percentage(100),
        "h-full" | "h-screen" => h.height = SizeHint::Percentage(100),
        "w-fit" | "w-auto-content" | "w-min" => h.width = SizeHint::Fit,
        "h-fit" | "h-auto-content" | "h-min" => h.height = SizeHint::Fit,
        "flex-1" | "grow" | "flex-grow" => {
            h.width = SizeHint::Fill;
            h.height = SizeHint::Fill;
        }
        "flex-none" | "shrink-0" => {} // no-op in terminal (no shrink concept)
        _ => return false,
    }
    true
}

fn apply_modifiers(class: &str, h: &mut StyleHints) -> bool {
    match class {
        "font-bold" | "bold" => h.modifiers |= Modifier::BOLD,
        "font-semibold" | "font-extrabold" | "font-black" => h.modifiers |= Modifier::BOLD,
        "italic" => h.modifiers |= Modifier::ITALIC,
        "underline" => h.modifiers |= Modifier::UNDERLINED,
        "line-through" => h.modifiers |= Modifier::CROSSED_OUT,
        "font-thin" | "font-light" | "dim" => h.modifiers |= Modifier::DIM,
        "blink" | "blink-slow" => h.modifiers |= Modifier::SLOW_BLINK,
        "blink-rapid" | "blink-fast" => h.modifiers |= Modifier::RAPID_BLINK,
        "overline" => {} // not supported in ratatui 0.29
        "hidden" | "invisible" => {
            // Mark as zero-size so it takes no space
            h.width = SizeHint::Fixed(0);
            h.height = SizeHint::Fixed(0);
        }
        "focusable" => h.focusable = true,
        _ => return false,
    }
    true
}

fn apply_borders(class: &str, h: &mut StyleHints) -> bool {
    match class {
        "border" => h.borders |= Borders::ALL,
        "border-t" | "border-top" => h.borders |= Borders::TOP,
        "border-b" | "border-bottom" => h.borders |= Borders::BOTTOM,
        "border-l" | "border-left" => h.borders |= Borders::LEFT,
        "border-r" | "border-right" => h.borders |= Borders::RIGHT,
        "border-x" => h.borders |= Borders::LEFT | Borders::RIGHT,
        "border-y" => h.borders |= Borders::TOP | Borders::BOTTOM,
        "border-none" => h.borders = Borders::NONE,
        "rounded" => h.border_type = BorderType::Rounded,
        "border-double" => h.border_type = BorderType::Double,
        "border-thick" => h.border_type = BorderType::Thick,
        "border-plain" => h.border_type = BorderType::Plain,
        _ => return false,
    }
    true
}

fn apply_alignment(class: &str, h: &mut StyleHints) -> bool {
    match class {
        "text-left" => h.alignment = Alignment::Left,
        "text-center" => h.alignment = Alignment::Center,
        "text-right" => h.alignment = Alignment::Right,
        "title-left" => h.title_alignment = Alignment::Left,
        "title-center" => h.title_alignment = Alignment::Center,
        "title-right" => h.title_alignment = Alignment::Right,
        _ => return false,
    }
    true
}

fn apply_named_colors(class: &str, h: &mut StyleHints) -> bool {
    match class {
        "text-white" => h.fg = Some(Color::White),
        "text-black" => h.fg = Some(Color::Black),
        "bg-white" => h.bg = Some(Color::White),
        "bg-black" => h.bg = Some(Color::Black),
        "bg-transparent" => h.bg = None,
        "tui-panel" => {
            h.borders |= Borders::ALL;
            h.border_type = BorderType::Rounded;
            h.border_fg = Some(Color::DarkGray);
            h.bg = Some(Color::Black);
            h.padding = Padding::uniform(1);
        }
        "tui-panel-bright" => {
            h.borders |= Borders::ALL;
            h.border_type = BorderType::Rounded;
            h.border_fg = Some(Color::Cyan);
            h.bg = Some(Color::Black);
            h.padding = Padding::uniform(1);
        }
        "tui-header" => {
            h.modifiers |= Modifier::BOLD;
            h.fg = Some(Color::LightCyan);
            h.bg = Some(Color::Black);
        }
        "tui-muted" => {
            h.modifiers |= Modifier::DIM;
            h.fg = Some(Color::DarkGray);
        }
        "tui-accent" => h.fg = Some(Color::LightMagenta),
        "tui-button" | "button" => {
            h.borders |= Borders::ALL;
            h.border_type = BorderType::Rounded;
            h.border_fg = Some(Color::Cyan);
            h.fg = Some(Color::White);
            h.bg = Some(Color::Black);
            h.padding = Padding::horizontal(1);
        }
        "tui-button-active" | "button-active" => {
            h.borders |= Borders::ALL;
            h.border_type = BorderType::Rounded;
            h.border_fg = Some(Color::LightCyan);
            h.fg = Some(Color::Black);
            h.bg = Some(Color::Cyan);
            h.padding = Padding::horizontal(1);
            h.modifiers |= Modifier::BOLD;
        }
        _ => return false,
    }
    true
}

fn apply_parametric(class: &str, h: &mut StyleHints) {
    if apply_focus_border(class, h) {
        return;
    }
    if apply_color_parametric(class, h) {
        return;
    }
    if apply_padding_parametric(class, h) {
        return;
    }
    apply_layout_parametric(class, h);
}

fn apply_focus_border(class: &str, h: &mut StyleHints) -> bool {
    if let Some(rest) = class.strip_prefix("focus:border-") {
        if let Some(c) = parse_color(rest) {
            h.focus_border_fg = Some(c);
            return true;
        }
    }
    false
}

fn apply_color_parametric(class: &str, h: &mut StyleHints) -> bool {
    if let Some(rest) = class.strip_prefix("text-") {
        if let Some(c) = parse_color(rest) {
            h.fg = Some(c);
            return true;
        }
    }
    if let Some(rest) = class.strip_prefix("bg-") {
        if let Some(c) = parse_color(rest) {
            h.bg = Some(c);
            return true;
        }
    }
    if let Some(rest) = class.strip_prefix("background-") {
        if let Some(c) = parse_color(rest) {
            h.bg = Some(c);
            return true;
        }
    }
    if let Some(rest) = class.strip_prefix("border-") {
        if let Some(c) = parse_color(rest) {
            h.border_fg = Some(c);
            return true;
        }
    }
    false
}

fn apply_padding_parametric(class: &str, h: &mut StyleHints) -> bool {
    if let Some(rest) = class.strip_prefix("p-") {
        if let Some(n) = parse_spacing(rest) {
            h.padding = Padding::uniform(n);
            return true;
        }
    }
    if let Some(rest) = class.strip_prefix("px-") {
        if let Some(n) = parse_spacing(rest) {
            h.padding = Padding::horizontal(n);
            return true;
        }
    }
    if let Some(rest) = class.strip_prefix("py-") {
        if let Some(n) = parse_spacing(rest) {
            h.padding = Padding::vertical(n);
            return true;
        }
    }
    if let Some(rest) = class.strip_prefix("pt-") {
        if let Some(n) = parse_spacing(rest) {
            let Padding {
                right,
                bottom,
                left,
                ..
            } = h.padding;
            h.padding = Padding::new(left, right, n, bottom);
            return true;
        }
    }
    if let Some(rest) = class.strip_prefix("pb-") {
        if let Some(n) = parse_spacing(rest) {
            let Padding {
                right, top, left, ..
            } = h.padding;
            h.padding = Padding::new(left, right, top, n);
            return true;
        }
    }
    if let Some(rest) = class.strip_prefix("pl-") {
        if let Some(n) = parse_spacing(rest) {
            let Padding {
                right, top, bottom, ..
            } = h.padding;
            h.padding = Padding::new(n, right, top, bottom);
            return true;
        }
    }
    if let Some(rest) = class.strip_prefix("pr-") {
        if let Some(n) = parse_spacing(rest) {
            let Padding {
                left, top, bottom, ..
            } = h.padding;
            h.padding = Padding::new(left, n, top, bottom);
            return true;
        }
    }
    false
}

fn apply_layout_parametric(class: &str, h: &mut StyleHints) -> bool {
    if let Some(rest) = class.strip_prefix("gap-") {
        if let Some(n) = parse_spacing(rest) {
            h.gap = n;
            return true;
        }
    }
    if let Some(rest) = class.strip_prefix("w-") {
        h.width = parse_size_hint(rest);
        return true;
    }
    if let Some(rest) = class.strip_prefix("h-") {
        h.height = parse_size_hint(rest);
        return true;
    }
    false
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Parse a spacing suffix: `1`, `2`, `[5]` → terminal cells (capped at 8).
fn parse_spacing(s: &str) -> Option<u16> {
    // Arbitrary value: p-[3]
    if let Some(inner) = s.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        return inner.parse::<u16>().ok().map(|n| n.min(16));
    }
    // Standard Tailwind scale: p-1 = 4px, but in terminal we map 1 unit = 1 cell
    s.parse::<u16>().ok().map(|n| n.min(8))
}

fn parse_size_hint(s: &str) -> SizeHint {
    match s {
        "full" | "screen" => return SizeHint::Percentage(100),
        "auto" => return SizeHint::Fill,
        "1/2" => return SizeHint::Percentage(50),
        "1/3" => return SizeHint::Percentage(33),
        "2/3" => return SizeHint::Percentage(67),
        "1/4" => return SizeHint::Percentage(25),
        "3/4" => return SizeHint::Percentage(75),
        "1/5" => return SizeHint::Percentage(20),
        "2/5" => return SizeHint::Percentage(40),
        "3/5" => return SizeHint::Percentage(60),
        "4/5" => return SizeHint::Percentage(80),
        _ => {}
    }
    // Arbitrary value: w-[40] (terminal cols) or w-[40%]
    if let Some(inner) = s.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        if let Some(pct) = inner.strip_suffix('%') {
            if let Ok(p) = pct.parse::<u16>() {
                return SizeHint::Percentage(p);
            }
        }
        if let Ok(n) = inner.parse::<u16>() {
            return SizeHint::Fixed(n);
        }
    }
    // Numeric: w-20 → 10 terminal cols (Tailwind uses 4px units; we halve for approximate terminal mapping)
    if let Ok(n) = s.parse::<u16>() {
        return SizeHint::Fixed((n / 2).max(1));
    }
    SizeHint::Fill
}

/// Parse a Tailwind colour spec into a ratatui `Color`.
///
/// Supported forms:
/// - Named + optional shade: `red-500`, `zinc-800`, `green`, `white`
/// - Hex (with or without brackets): `[#ff5733]`, `#aabbcc`
/// - ANSI index: `[ansi-196]`
pub fn parse_color(s: &str) -> Option<Color> {
    // Strip arbitrary-value brackets
    let s = s.trim_matches(|c| c == '[' || c == ']');

    // ANSI 256-colour index: ansi-196
    if let Some(idx) = s.strip_prefix("ansi-") {
        if let Ok(n) = idx.parse::<u8>() {
            return Some(Color::Indexed(n));
        }
    }

    // Hex colour
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            return Some(Color::Rgb(r, g, b));
        }
    }

    // Named + optional shade (e.g. "red-500", "zinc-800", "green")
    let (family, shade) = split_color_shade(s);
    // Shades ≤ 400 are lighter; > 400 are darker (terminal "Light*" = brighter ANSI)
    let light = shade.is_some_and(|n| n <= 400);

    match family {
        "white" => Some(Color::White),
        "black" => Some(Color::Black),
        "red" | "rose" => Some(if light { Color::LightRed } else { Color::Red }),
        "green" | "emerald" | "lime" => Some(if light {
            Color::LightGreen
        } else {
            Color::Green
        }),
        "teal" | "cyan" | "sky" => Some(if light { Color::LightCyan } else { Color::Cyan }),
        "blue" | "indigo" => Some(if light { Color::LightBlue } else { Color::Blue }),
        "yellow" | "amber" => Some(if light {
            Color::LightYellow
        } else {
            Color::Yellow
        }),
        "orange" => Some(Color::LightYellow), // terminal doesn't have a distinct orange
        "purple" | "violet" | "fuchsia" | "pink" | "magenta" => Some(if light {
            Color::LightMagenta
        } else {
            Color::Magenta
        }),
        "gray" | "grey" | "zinc" | "slate" | "neutral" | "stone" => {
            Some(match shade.unwrap_or(500) {
                n if n <= 400 => Color::Gray,
                _ => Color::DarkGray,
            })
        }
        _ => None,
    }
}

fn split_color_shade(s: &str) -> (&str, Option<u16>) {
    if let Some(pos) = s.rfind('-') {
        let suffix = &s[pos + 1..];
        if suffix.bytes().all(|b| b.is_ascii_digit()) {
            if let Ok(n) = suffix.parse::<u16>() {
                return (&s[..pos], Some(n));
            }
        }
    }
    (s, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color() {
        // ANSI indexed
        assert_eq!(parse_color("ansi-196"), Some(Color::Indexed(196)));
        assert_eq!(parse_color("ansi-0"), Some(Color::Indexed(0)));
        assert_eq!(parse_color("ansi-255"), Some(Color::Indexed(255)));
        assert_eq!(parse_color("ansi-256"), None);

        // Hex colours
        assert_eq!(parse_color("#ff0000"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_color("#00ff00"), Some(Color::Rgb(0, 255, 0)));
        assert_eq!(parse_color("#0000ff"), Some(Color::Rgb(0, 0, 255)));
        assert_eq!(parse_color("#123"), None);
        assert_eq!(parse_color("#ff0000ff"), None);
        assert_eq!(parse_color("#zx0000"), None);

        // Arbitrary-value brackets
        assert_eq!(parse_color("[#ff0000]"), Some(Color::Rgb(255, 0, 0)));
        assert_eq!(parse_color("[ansi-12]"), Some(Color::Indexed(12)));
        assert_eq!(parse_color("[red-500]"), Some(Color::Red));

        // Named colours
        assert_eq!(parse_color("white"), Some(Color::White));
        assert_eq!(parse_color("black"), Some(Color::Black));

        // Named colours with shades
        assert_eq!(parse_color("red-500"), Some(Color::Red));
        assert_eq!(parse_color("red-400"), Some(Color::LightRed));
        assert_eq!(parse_color("green-500"), Some(Color::Green));
        assert_eq!(parse_color("lime-300"), Some(Color::LightGreen));
        assert_eq!(parse_color("blue-600"), Some(Color::Blue));
        assert_eq!(parse_color("sky-200"), Some(Color::LightCyan));
        assert_eq!(parse_color("yellow"), Some(Color::Yellow));
        assert_eq!(parse_color("amber-300"), Some(Color::LightYellow));
        assert_eq!(parse_color("orange-500"), Some(Color::LightYellow));
        assert_eq!(parse_color("purple-800"), Some(Color::Magenta));
        assert_eq!(parse_color("fuchsia-100"), Some(Color::LightMagenta));

        // Grays
        assert_eq!(parse_color("gray-500"), Some(Color::DarkGray));
        assert_eq!(parse_color("zinc-400"), Some(Color::Gray));
        assert_eq!(parse_color("slate-900"), Some(Color::DarkGray));
        assert_eq!(parse_color("neutral"), Some(Color::DarkGray));

        // Invalid
        assert_eq!(parse_color("unknown"), None);
        assert_eq!(parse_color("red-xx"), None);
    }

    #[test]
    fn parse_classes_happy_path() {
        let classes = vec![
            "bg-red-500".to_string(),
            "text-white".to_string(),
            "font-bold".to_string(),
            "p-2".to_string(),
            "w-full".to_string(),
            "flex-col".to_string(),
            "border".to_string(),
            "gap-2".to_string(),
        ];

        let hints = parse_classes(&classes);

        assert_eq!(hints.bg, Some(Color::Red));
        assert_eq!(hints.fg, Some(Color::White));
        assert!(hints.modifiers.contains(Modifier::BOLD));
        assert_eq!(hints.padding.left, 2);
        assert_eq!(hints.padding.right, 2);
        assert_eq!(hints.padding.top, 2);
        assert_eq!(hints.padding.bottom, 2);
        assert_eq!(hints.direction, Direction::Vertical);
        assert_eq!(hints.borders, Borders::ALL);
        assert_eq!(hints.gap, 2);

        match hints.width {
            SizeHint::Percentage(100) => {}
            _ => panic!("Expected width to be Percentage(100)"),
        }
    }

    #[test]
    fn parse_classes_empty_input() {
        let hints = parse_classes(&[]);

        assert_eq!(hints.bg, None);
        assert_eq!(hints.fg, None);
        assert!(hints.modifiers.is_empty());
        assert_eq!(hints.padding.left, 0);
        assert_eq!(hints.direction, Direction::Vertical);
        assert_eq!(hints.borders, Borders::NONE);
        assert_eq!(hints.gap, 0);

        match hints.width {
            SizeHint::Fill => {}
            _ => panic!("Expected width to be Fill by default"),
        }
    }

    #[test]
    fn parse_classes_invalid_classes() {
        let classes = vec!["not-a-tailwind-class".to_string(), "fake-style".to_string()];

        let hints = parse_classes(&classes);

        assert_eq!(hints.bg, None);
        assert_eq!(hints.fg, None);
        assert!(hints.modifiers.is_empty());
        assert_eq!(hints.padding.left, 0);
        assert_eq!(hints.direction, Direction::Vertical);
        assert_eq!(hints.borders, Borders::NONE);
        assert_eq!(hints.gap, 0);

        match hints.width {
            SizeHint::Fill => {}
            _ => panic!("Expected width to be Fill by default"),
        }
    }
}
