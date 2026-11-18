/// Pure-Dart, indentation-based `.crepus` parser producing the same [ViewIr]
/// model the IR decoder yields. Spec references: `tree-sitter-crepus/grammar.js`
/// (line + class-segment shape) and `crates/crepuscularity-native/src/render.rs`
/// (tag → `ViewNode` mapping). This is a CONSTRAINED reimplementation: only the
/// allowlisted vocabulary is produced; disallowed tags become [UnsupportedNode].
///
/// Interpolation (`{expr}`) is intentionally NOT resolved here — the renderer
/// substitutes against a runtime scope so `forEach` item bindings work. There is
/// no expression evaluation anywhere in this parser.
library;

import 'view_ir.dart';

/// Parse `.crepus` source into a [ViewIr]. Oversized source (beyond
/// [CrepusLimits.maxSourceLength]) or a document that breaks the node/depth caps
/// yields an empty root — never an exception.
ViewIr viewIrFromSource(
  String source, {
  CrepusLimits limits = CrepusLimits.defaults,
}) {
  if (source.length > limits.maxSourceLength) {
    return const ViewIr(version: IR_VERSION, root: []);
  }
  final lines = _lex(source);
  if (lines.isEmpty) {
    return const ViewIr(version: IR_VERSION, root: []);
  }
  final parser = _TreeBuilder(lines);
  final raws = parser.parseBlock(-1);
  final root = _mapSiblings(raws);
  if (crepusExceedsLimits(root, limits)) {
    return const ViewIr(version: IR_VERSION, root: []);
  }
  return ViewIr(version: IR_VERSION, root: root);
}

// The View IR version this parser targets; kept in step with `ir.rs`.
// ignore: constant_identifier_names
const int IR_VERSION = 7;

// ── Lexing: one significant line → indent + tokens ──────────────────────────

class _Line {
  _Line(this.indent, this.tokens);
  final int indent;
  final List<String> tokens; // first token is the tag; a bare `"..."` is text
}

List<_Line> _lex(String source) {
  final out = <_Line>[];
  for (final raw in source.split('\n')) {
    var indent = 0;
    while (indent < raw.length &&
        (raw.codeUnitAt(indent) == 0x20 || raw.codeUnitAt(indent) == 0x09)) {
      indent += 1;
    }
    final content = raw.substring(indent);
    final trimmed = content.trimRight();
    if (trimmed.isEmpty) continue; // blank
    if (trimmed.startsWith('#')) continue; // comment
    if (trimmed == '+++' || trimmed.startsWith('---')) continue; // frontmatter
    final tokens = _tokenize(trimmed);
    if (tokens.isEmpty) continue;
    out.add(_Line(indent, tokens));
  }
  return out;
}

/// Split a line into whitespace-delimited tokens, but keep `"..."` strings and
/// `{...}` expressions whole (so `onclick={prompt:do it}` stays one token).
List<String> _tokenize(String line) {
  final tokens = <String>[];
  final n = line.length;
  var i = 0;
  while (i < n) {
    final c = line.codeUnitAt(i);
    if (c == 0x20 || c == 0x09) {
      i += 1;
      continue;
    }
    final sb = StringBuffer();
    while (i < n) {
      final ch = line.codeUnitAt(i);
      if (ch == 0x20 || ch == 0x09) break;
      if (ch == 0x22) {
        // opening quote — consume through the closing quote (honoring escapes)
        sb.writeCharCode(ch);
        i += 1;
        while (i < n) {
          final q = line.codeUnitAt(i);
          sb.writeCharCode(q);
          i += 1;
          if (q == 0x5c && i < n) {
            sb.writeCharCode(line.codeUnitAt(i));
            i += 1;
            continue;
          }
          if (q == 0x22) break;
        }
        continue;
      }
      if (ch == 0x7b) {
        // `{` — consume through `}` (no nesting, matching the grammar)
        sb.writeCharCode(ch);
        i += 1;
        while (i < n && line.codeUnitAt(i) != 0x7d) {
          sb.writeCharCode(line.codeUnitAt(i));
          i += 1;
        }
        if (i < n) {
          sb.writeCharCode(0x7d);
          i += 1;
        }
        continue;
      }
      sb.writeCharCode(ch);
      i += 1;
    }
    tokens.add(sb.toString());
  }
  return tokens;
}

// ── Tree building by indentation ────────────────────────────────────────────

class _Raw {
  _Raw(this.line);
  final _Line line;
  final List<_Raw> children = [];
}

class _TreeBuilder {
  _TreeBuilder(this.lines);
  final List<_Line> lines;
  int _pos = 0;

  /// Parse every node whose indent is greater than [minIndent].
  List<_Raw> parseBlock(int minIndent) {
    final nodes = <_Raw>[];
    while (_pos < lines.length && lines[_pos].indent > minIndent) {
      final line = lines[_pos];
      _pos += 1;
      final node = _Raw(line);
      node.children.addAll(parseBlock(line.indent));
      nodes.add(node);
    }
    return nodes;
  }
}

// ── Mapping raw lines → constrained ViewNodes ───────────────────────────────

const Set<String> _textTags = {
  'text',
  'span',
  'p',
  'label',
  'caption',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
};

List<ViewNode> _mapSiblings(List<_Raw> raws) {
  final out = <ViewNode>[];
  for (var i = 0; i < raws.length; i++) {
    final raw = raws[i];
    final tag = _tag(raw).toLowerCase();
    if (tag == 'if') {
      // An immediately-following `else` sibling supplies the else-branch.
      List<_Raw>? elseRaws;
      if (i + 1 < raws.length && _tag(raws[i + 1]).toLowerCase() == 'else') {
        elseRaws = raws[i + 1].children;
        i += 1;
      }
      out.add(_mapIf(raw, elseRaws));
      continue;
    }
    if (tag == 'else') {
      // A stray `else` (no preceding `if`) is inert.
      continue;
    }
    out.add(_mapNode(raw));
  }
  return out;
}

String _tag(_Raw raw) => raw.line.tokens.first;

/// A bare `"..."` line is a text node.
bool _isTextLine(_Raw raw) =>
    raw.line.tokens.length == 1 && raw.line.tokens.first.startsWith('"');

ViewNode _mapNode(_Raw raw) {
  if (_isTextLine(raw)) {
    return TextNode(content: _unquote(raw.line.tokens.first));
  }
  final el = _Element.parse(raw);
  final tag = el.tag.toLowerCase();

  // Disallowed / executing tags — inert. Listed explicitly so the boundary is
  // greppable; anything not matched below also falls through to Unsupported.
  const disallowed = {
    'iframe',
    'webview',
    'input',
    'textfield',
    'textinput',
    'textarea',
    'picker',
    'select',
    'slider',
    'tabs',
    'tabview',
    'page-switcher',
    'dropzone',
    'file-picker',
    'filepicker',
    'media-picker',
    'slot',
    'slot-rotate',
    'embed',
  };
  if (disallowed.contains(tag)) return UnsupportedNode(tag);

  final style = styleFromClasses(el.classes, id: el.id);

  if (_textTags.contains(tag)) {
    final content = el.inlineText ?? _firstTextChild(el);
    if (content != null) {
      return TextNode(content: content, bind: el.attrs['bind'], style: style);
    }
  }

  switch (tag) {
    case 'button':
      return ButtonNode(
        label: el.label(),
        onClick: el.events['click'],
        onLongPress: el.events['longpress'],
        style: style,
      );
    case 'toggle':
    case 'switch':
      return ToggleNode(
        label: el.label(),
        bind: el.attrs['bind'],
        checked: el.flag('checked'),
        onChange: el.events['change'],
        onLongPress: el.events['longpress'],
        style: style,
      );
    case 'checkbox':
      return CheckboxNode(
        label: el.label(),
        bind: el.attrs['bind'],
        checked: el.flag('checked'),
        onChange: el.events['change'],
        style: style,
      );
    case 'progress':
      return ProgressNode(
        label: el.optionalLabel(),
        value: el.number('value') ?? 0,
        max: el.number('max') ?? 100,
        style: style,
      );
    case 'timer':
      return TimerNode(
        label: el.optionalLabel(),
        duration: parseCrepusDuration(el.attrs['duration']),
        autostart: el.flag('autostart'),
        countUp: el.flag('countup'),
        style: style,
      );
    case 'meter':
      return MeterNode(
        label: el.optionalLabel(),
        value: el.number('value') ?? 0,
        min: el.number('min') ?? 0,
        max: el.number('max') ?? 100,
        style: style,
      );
    case 'sparkline':
      return SparklineNode(
        values: _parseSparklineValues(el.attrs['values']),
        color: el.attrs['color'],
        variant: el.attrs['variant'],
        style: style,
      );
    case 'badge':
      return BadgeNode(label: el.label(), tone: el.attrs['tone'], style: style);
    case 'divider':
    case 'hr':
      return DividerNode(axis: _axisOf(el.classes), style: style);
    case 'spacer':
      return SpacerNode(
        size: el.number('size') ?? _gap(el.classes),
        style: style,
      );
    case 'img':
    case 'image':
      return ImageNode(
        src: el.attrs['src'] ?? '',
        alt: el.attrs['alt'],
        onLongPress: el.events['longpress'],
        style: style,
      );
    case 'ul':
    case 'ol':
    case 'list':
    case 'flatlist':
      return ListNode(
        ordered: tag == 'ol',
        children: _mapSiblings(raw.children),
        style: style,
      );
    case 'a':
    case 'link':
      return LinkNode(
        href: el.attrs['href'] ?? '',
        target: el.attrs['target'],
        rel: el.attrs['rel'],
        children: _listItemChildren(raw, el, style),
        style: style,
      );
    case 'li':
    case 'list-item':
    case 'listitem':
      return ListItemNode(
        onClick: el.events['click'],
        onLongPress: el.events['longpress'],
        children: _listItemChildren(raw, el, style),
        style: style,
      );
    case 'stack':
    case 'card':
    case 'section':
    case 'panel':
    case 'group':
    case 'view':
      return StackNode(
        axis: tag == 'view'
            ? StackAxis.column
            : (tag == 'row' ? StackAxis.row : _axisOf(el.classes)),
        spacing: _gap(el.classes),
        alignItems: _alignItems(el.classes),
        justifyContent: _justify(el.classes),
        onLongPress: el.events['longpress'],
        children: _mapSiblings(raw.children),
        style: style,
      );
    case 'for':
    case 'foreach':
      return _mapFor(el, raw);
  }

  // Fallback: a container → stack (or scroll wrapper).
  final axis = tag == 'view' ? StackAxis.column : _axisOf(el.classes);
  final children = _mapSiblings(raw.children);
  if (tag == 'scroll' || _isScroll(el.classes)) {
    return ScrollNode(axis: axis, children: children, style: style);
  }
  return StackNode(
    axis: axis,
    spacing: _gap(el.classes),
    alignItems: _alignItems(el.classes),
    justifyContent: _justify(el.classes),
    onLongPress: el.events['longpress'],
    children: children,
    style: style,
  );
}

ViewNode _mapIf(_Raw raw, List<_Raw>? elseRaws) {
  // Condition = the tokens after the `if` tag, rejoined (evaluated safely by
  // the renderer; never `eval`ed).
  final condition = raw.line.tokens.skip(1).join(' ');
  return IfNode(
    condition: condition,
    thenChildren: _mapSiblings(raw.children),
    elseChildren: elseRaws == null ? null : _mapSiblings(elseRaws),
  );
}

ViewNode _mapFor(_Element el, _Raw raw) {
  // Accept `for <item> in <list>` and `foreach <list> as <item>`.
  final tokens = raw.line.tokens.skip(1).toList();
  var itemName = 'item';
  var bind = '';
  final inIdx = tokens.indexOf('in');
  final asIdx = tokens.indexOf('as');
  if (inIdx > 0 && inIdx + 1 < tokens.length) {
    itemName = tokens[inIdx - 1];
    bind = tokens[inIdx + 1];
  } else if (asIdx > 0 && asIdx + 1 < tokens.length) {
    bind = tokens[asIdx - 1];
    itemName = tokens[asIdx + 1];
  } else if (tokens.isNotEmpty) {
    bind = tokens.first;
  }
  return ForEachNode(
    bind: bind,
    itemName: itemName,
    itemBody: _mapSiblings(raw.children),
    style: styleFromClasses(el.classes, id: el.id),
  );
}

String? _firstTextChild(_Element el) {
  for (final child in el.raw.children) {
    if (_isTextLine(child)) return _unquote(child.line.tokens.first);
  }
  return null;
}

/// Inline `"..."` / `label=` on a list item becomes a text child when no
/// indented children were authored — matching how labels work on buttons.
List<ViewNode> _listItemChildren(_Raw raw, _Element el, ViewStyle? style) {
  final mapped = _mapSiblings(raw.children);
  if (mapped.isNotEmpty) return mapped;
  final bind = el.attrs['bind'];
  if (bind != null && bind.isNotEmpty) {
    return [TextNode(content: '', bind: bind, style: style)];
  }
  final label = el.optionalLabel();
  if (label != null && label.isNotEmpty) {
    return [TextNode(content: label, style: style)];
  }
  return const [];
}

// ── Parsed element (tag + classes + attrs + events + inline text) ───────────

class _Element {
  _Element(this.raw, this.tag);
  final _Raw raw;
  final String tag;
  final List<String> classes = [];
  final Map<String, String> attrs = {};
  final Map<String, String> events = {};
  String? inlineText;
  String? id;

  static _Element parse(_Raw raw) {
    final el = _Element(raw, raw.line.tokens.first);
    for (final token in raw.line.tokens.skip(1)) {
      if (token.startsWith('"')) {
        el.inlineText = _unquote(token);
        continue;
      }
      if (token.startsWith('#')) {
        final id = token.substring(1);
        if (id.isNotEmpty) el.id = id;
        continue;
      }
      final eq = token.indexOf('=');
      if (eq > 0) {
        final name = token.substring(0, eq);
        final value = _attrValue(token.substring(eq + 1));
        final ev = _eventName(name);
        if (ev != null) {
          el.events[ev] = value;
        } else {
          el.attrs[name] = value;
        }
        continue;
      }
      el.classes.add(token);
    }
    return el;
  }

  bool flag(String name) {
    if (classes.contains(name)) return true;
    final value = attrs[name];
    if (value == null) return false;
    final lower = value.trim().toLowerCase();
    return !(lower.isEmpty ||
        lower == 'false' ||
        lower == '0' ||
        lower == 'no' ||
        lower == 'off');
  }

  double? number(String key) {
    final value = attrs[key];
    if (value == null) return null;
    final parsed = double.tryParse(value);
    return parsed != null && parsed.isFinite ? parsed : null;
  }

  String label() => optionalLabel() ?? '';

  String? optionalLabel() {
    if (attrs['label'] != null) return attrs['label'];
    if (inlineText != null) return inlineText;
    return _firstTextChild(this);
  }
}

/// `onclick`/`on-change`/`onLongPress` → `click`/`change`/`longpress`.
/// Anything not starting with `on` is a plain attribute, not an event.
String? _eventName(String attr) {
  final lower = attr.toLowerCase();
  if (!lower.startsWith('on')) return null;
  var rest = lower.substring(2);
  if (rest.startsWith('-') || rest.startsWith('_')) rest = rest.substring(1);
  return rest.replaceAll('-', '').replaceAll('_', '');
}

String _attrValue(String raw) {
  if (raw.startsWith('"')) return _unquote(raw);
  if (raw.startsWith('{') && raw.endsWith('}')) {
    return raw.substring(1, raw.length - 1).trim();
  }
  return raw;
}

String _unquote(String raw) {
  var s = raw;
  if (s.startsWith('"')) s = s.substring(1);
  if (s.endsWith('"')) s = s.substring(0, s.length - 1);
  return s
      .replaceAll('\\n', '\n')
      .replaceAll('\\t', '\t')
      .replaceAll('\\"', '"')
      .replaceAll('\\\\', '\\');
}

// ── Tailwind-ish class → style/layout hints (subset of style.rs) ────────────

StackAxis _axisOf(List<String> classes) {
  // Both Tailwind (`flex-col`/`flex-row`) and the terse `stack col`/`stack row`
  // shorthand are accepted.
  if (classes.contains('flex-col') ||
      classes.contains('col') ||
      classes.contains('column')) {
    return StackAxis.column;
  }
  if (classes.contains('flex-row') ||
      classes.contains('row') ||
      classes.contains('flex')) {
    return StackAxis.row;
  }
  return StackAxis.column;
}

bool _isScroll(List<String> classes) =>
    classes.contains('overflow-scroll') ||
    classes.contains('overflow-auto') ||
    classes.contains('overflow-y-auto') ||
    classes.contains('overflow-y-scroll') ||
    classes.contains('scroll');

double? _gap(List<String> classes) {
  for (final c in classes) {
    if (c.startsWith('gap-')) {
      final n = int.tryParse(c.substring(4));
      if (n != null) return n * 4.0;
    }
  }
  return null;
}

String? _alignItems(List<String> classes) {
  if (classes.contains('items-center')) return 'center';
  if (classes.contains('items-end')) return 'end';
  if (classes.contains('items-start')) return 'start';
  if (classes.contains('items-stretch')) return 'stretch';
  return null;
}

String? _justify(List<String> classes) {
  if (classes.contains('justify-center')) return 'center';
  if (classes.contains('justify-end')) return 'end';
  if (classes.contains('justify-between')) return 'between';
  if (classes.contains('justify-around')) return 'around';
  if (classes.contains('justify-start')) return 'start';
  return null;
}

const Map<String, double> _fontSizes = {
  'text-xs': 12,
  'text-sm': 13,
  'text-base': 15,
  'text-lg': 17,
  'text-xl': 20,
  'text-2xl': 24,
  'text-3xl': 30,
};

const Map<String, int> _fontWeights = {
  'font-thin': 100,
  'font-light': 300,
  'font-normal': 400,
  'font-medium': 500,
  'font-semibold': 600,
  'font-bold': 700,
  'font-extrabold': 800,
  'font-black': 900,
};

/// Build a [ViewStyle] from Tailwind-ish utility classes. Only the classes the
/// renderer honors are recognized; unknown classes are ignored.
ViewStyle? styleFromClasses(List<String> classes, {String? id}) {
  double? padding;
  double? px;
  double? py;
  double? pt;
  double? pb;
  double? pl;
  double? pr;
  double? fontSize;
  int? fontWeight;
  String? textAlign;
  String? fg;
  String? bg;
  double? radius;
  bool? italic;
  bool? underline;
  bool? strikethrough;

  double? spacingUnit(String rest) {
    final n = int.tryParse(rest);
    return n == null ? null : n * 4.0;
  }

  for (final c in classes) {
    if (_fontSizes.containsKey(c)) {
      fontSize = _fontSizes[c];
      continue;
    }
    if (_fontWeights.containsKey(c)) {
      fontWeight = _fontWeights[c];
      continue;
    }
    if (c == 'italic') {
      italic = true;
      continue;
    }
    if (c == 'underline') {
      underline = true;
      continue;
    }
    if (c == 'line-through') {
      strikethrough = true;
      continue;
    }
    if (c == 'text-left') textAlign = 'left';
    if (c == 'text-center') textAlign = 'center';
    if (c == 'text-right') textAlign = 'right';
    if (c.startsWith('p-')) padding = spacingUnit(c.substring(2)) ?? padding;
    if (c.startsWith('px-')) px = spacingUnit(c.substring(3)) ?? px;
    if (c.startsWith('py-')) py = spacingUnit(c.substring(3)) ?? py;
    if (c.startsWith('pt-')) pt = spacingUnit(c.substring(3)) ?? pt;
    if (c.startsWith('pb-')) pb = spacingUnit(c.substring(3)) ?? pb;
    if (c.startsWith('pl-')) pl = spacingUnit(c.substring(3)) ?? pl;
    if (c.startsWith('pr-')) pr = spacingUnit(c.substring(3)) ?? pr;
    if (c.startsWith('rounded')) {
      radius = _roundedValue(c);
    }
    final color = _colorClass(c);
    if (color != null) {
      if (color.$1 == 'text') fg = color.$2;
      if (color.$1 == 'bg') bg = color.$2;
    }
  }

  final style = ViewStyle(
    id: id,
    classes: List<String>.of(classes),
    padding: padding,
    paddingHorizontal: px,
    paddingVertical: py,
    paddingTop: pt,
    paddingBottom: pb,
    paddingLeft: pl,
    paddingRight: pr,
    fontSize: fontSize,
    fontWeight: fontWeight,
    textAlign: textAlign,
    foregroundColor: fg,
    backgroundColor: bg,
    cornerRadius: radius,
    italic: italic,
    underline: underline,
    strikethrough: strikethrough,
  );
  return style.isEmpty ? null : style;
}

double _roundedValue(String c) {
  switch (c) {
    case 'rounded-none':
      return 0;
    case 'rounded-sm':
      return 4;
    case 'rounded':
      return 8;
    case 'rounded-md':
      return 10;
    case 'rounded-lg':
      return 12;
    case 'rounded-xl':
      return 16;
    case 'rounded-2xl':
      return 20;
    case 'rounded-full':
      return 999;
  }
  return 8;
}

/// Recognize a small, fixed palette of `text-*` / `bg-*` / `border-*` classes
/// and return a hex string. Only named tokens map — no arbitrary values.
(String, String)? _colorClass(String c) {
  for (final prefix in const ['text-', 'bg-']) {
    if (c.startsWith(prefix)) {
      final name = c.substring(prefix.length);
      final hex = _palette[name];
      if (hex != null) return (prefix.substring(0, prefix.length - 1), hex);
    }
  }
  return null;
}

const Map<String, String> _palette = {
  'white': '#ffffff',
  'black': '#000000',
  'transparent': '#00000000',
  'muted': '#8d8980',
  'red-500': '#fb2c36',
  'green-500': '#00c950',
  'blue-500': '#2b7fff',
  'blue-900': '#1c398e',
  'amber-500': '#fe9a00',
  'slate-500': '#62748e',
};

List<double> _parseSparklineValues(String? raw) {
  if (raw == null || raw.trim().isEmpty) return const [];
  final cleaned = raw.replaceAll('[', '').replaceAll(']', '');
  return cleaned
      .split(RegExp(r'[,\s]+'))
      .map((part) => double.tryParse(part.trim()))
      .whereType<double>()
      .where((value) => value.isFinite)
      .toList(growable: false);
}
