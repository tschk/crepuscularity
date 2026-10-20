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
import 'string_literals.dart';
import 'color_literals.dart';

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

// Match nested braces while keeping braces inside quoted expressions opaque.
int? _closingBrace(String text, int start) {
  var depth = 1;
  int? quote;
  for (var i = start + 1; i < text.length; i++) {
    final next = text.codeUnitAt(i);
    if (quote != null) {
      if (next == 0x5c && i + 1 < text.length) {
        i++;
      } else if (next == quote) {
        quote = null;
      }
    } else if (next == 0x22 || next == 0x27) {
      quote = next;
    } else if (next == 0x7b) {
      depth++;
    } else if (next == 0x7d && --depth == 0) {
      return i;
    }
  }
  return null;
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
        // Only standalone text tokens contain interpolation; attributes are literal.
        final interpolate = sb.isEmpty;
        sb.writeCharCode(ch);
        i += 1;
        while (i < n) {
          final q = line.codeUnitAt(i);
          if (interpolate && q == 0x7b) {
            final end = _closingBrace(line, i);
            sb.write(line.substring(i, end == null ? n : end + 1));
            i = end == null ? n : end + 1;
            continue;
          }
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
        final end = _closingBrace(line, i);
        sb.write(line.substring(i, end == null ? n : end + 1));
        i = end == null ? n : end + 1;
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
    return TextNode(content: unquoteCrepusLiteral(raw.line.tokens.first));
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
        labelParts: el.buttonLabelParts(),
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
        srcBind: el.urlBindings['src'],
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
        hrefBind: el.urlBindings['href'],
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
    case 'for':
    case 'foreach':
      return _mapFor(el, raw);
  }

  // Fallback: a container → stack (or scroll wrapper).
  final axis = tag == 'view' ? StackAxis.column : _axisOf(el.classes);
  final children = _mapSiblings(raw.children);
  final scrollAxis = _scrollAxis(el.classes, axis);
  if (tag == 'scroll' || scrollAxis != null) {
    return ScrollNode(
      axis: scrollAxis ?? axis,
      children: [
        StackNode(
          axis: axis,
          spacing: _gap(el.classes),
          alignItems: _alignItems(el.classes),
          justifyContent: _justify(el.classes),
          children: children,
        ),
      ],
      style: style,
      onLongPress: el.events['longpress'],
    );
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
    if (_isTextLine(child))
      return unquoteCrepusLiteral(child.line.tokens.first);
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
  final Map<String, String> urlBindings = {};
  final Map<String, String> events = {};
  String? inlineText;
  String? inlineTextToken;
  String? id;

  static _Element parse(_Raw raw) {
    final el = _Element(raw, raw.line.tokens.first);
    for (final token in raw.line.tokens.skip(1)) {
      if (token.startsWith('"')) {
        el.inlineTextToken = token;
        el.inlineText = unquoteCrepusLiteral(token);
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
        final rawValue = token.substring(eq + 1);
        final value = _attrValue(rawValue);
        final ev = _eventName(name);
        if (ev != null) {
          el.events[ev] = value;
        } else {
          el.attrs[name] = value;
          if (name == 'src' || name == 'href') {
            // Preserve expression origin; quoted URLs with braces are literal.
            el.urlBindings.remove(name);
            if (rawValue.startsWith('{') &&
                _closingBrace(rawValue, 0) == rawValue.length - 1) {
              final expression = rawValue
                  .substring(1, rawValue.length - 1)
                  .trim();
              if (expression.isNotEmpty) el.urlBindings[name] = expression;
            }
          }
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

  List<ButtonLabelPart> buttonLabelParts() {
    if (attrs.containsKey('label')) return const [];
    var token = inlineTextToken;
    if (token == null) {
      for (final child in raw.children) {
        if (_isTextLine(child)) {
          token = child.line.tokens.first;
          break;
        }
      }
    }
    if (token == null || token.length < 2 || !token.endsWith('"'))
      return const [];
    final content = token.substring(1, token.length - 1);
    if (!content.contains('{')) return const [];
    final parts = <ButtonLabelPart>[];
    var offset = 0;
    while (offset < content.length) {
      final start = content.indexOf('{', offset);
      if (start < 0) {
        parts.add(
          ButtonLabelPart(
            kind: 'literal',
            value: unquoteCrepusLiteral('"${content.substring(offset)}"'),
          ),
        );
        break;
      }
      if (start > offset) {
        parts.add(
          ButtonLabelPart(
            kind: 'literal',
            value: unquoteCrepusLiteral(
              '"${content.substring(offset, start)}"',
            ),
          ),
        );
      }
      final end = _closingBrace(content, start);
      if (end == null) return const [];
      final expression = content.substring(start + 1, end).trim();
      if (expression.isEmpty) return const [];
      parts.add(ButtonLabelPart(kind: 'expr', value: expression));
      offset = end + 1;
    }
    return parts;
  }

  String label() => optionalLabel() ?? '';

  String? optionalLabel() {
    if (attrs['label'] != null) return attrs['label'];
    if (inlineText != null) return inlineText;
    return _firstTextChild(this);
  }
}

/// `onclick`/`on-change`/`onLongPress` → `click`/`change`/`longpress`.
/// `@click` uses the canonical event prefix; other names remain attributes.
String? _eventName(String attr) {
  final lower = attr.toLowerCase();
  final prefixLength = lower.startsWith('@')
      ? 1
      : lower.startsWith('on')
      ? 2
      : 0;
  if (prefixLength == 0) return null;
  var rest = lower.substring(prefixLength);
  if (rest.startsWith('-') || rest.startsWith('_')) rest = rest.substring(1);
  return rest.replaceAll('-', '').replaceAll('_', '');
}

String _attrValue(String raw) {
  if (raw.startsWith('"')) return unquoteCrepusLiteral(raw);
  if (raw.startsWith('{') && raw.endsWith('}')) {
    return raw.substring(1, raw.length - 1).trim();
  }
  return raw;
}

// ── Tailwind-ish class → style/layout hints (subset of style.rs) ────────────

StackAxis _axisOf(List<String> classes) {
  // Both Tailwind (`flex-col`/`flex-row`) and the terse `stack col`/`stack row`
  // shorthand are accepted.
  for (final c in classes.reversed) {
    if (c == 'flex-col' || c == 'col' || c == 'column') {
      return StackAxis.column;
    }
    if (c == 'flex-row' || c == 'row') return StackAxis.row;
  }
  return classes.contains('flex') ? StackAxis.row : StackAxis.column;
}

StackAxis? _scrollAxis(List<String> classes, StackAxis layoutAxis) {
  final horizontal = classes.any(
    (c) => c == 'overflow-x-auto' || c == 'overflow-x-scroll',
  );
  final vertical = classes.any(
    (c) => c == 'overflow-y-auto' || c == 'overflow-y-scroll',
  );
  if (horizontal && !vertical) return StackAxis.row;
  if (vertical && !horizontal) return StackAxis.column;
  if (horizontal ||
      vertical ||
      classes.any(
        (c) => c == 'overflow-auto' || c == 'overflow-scroll' || c == 'scroll',
      ))
    return layoutAxis;
  return null;
}

double? _spacingUnit(String value) {
  var scale = 4.0;
  if (value.startsWith('[') && value.endsWith(']')) {
    value = value.substring(1, value.length - 1);
    if (value.endsWith('px')) value = value.substring(0, value.length - 2);
    scale = 1.0;
  }
  final number = double.tryParse(value);
  if (number == null) return null;
  final pixels = number * scale;
  return pixels.isFinite ? pixels : null;
}

/// One margin arm. A leading `-` negates the resolved spacing (`-m-4`, `-mx-2`).
/// The named `px` step is 1; every other token goes through [_spacingUnit].
/// A token that does not parse leaves [current] unchanged.
double? _marginClass(String token, String prefix, double? current) {
  final negative = token.startsWith('-');
  final body = negative ? token.substring(1) : token;
  if (!body.startsWith(prefix)) return current;
  final raw = body.substring(prefix.length);
  final spacing = raw == 'px' ? 1.0 : _spacingUnit(raw);
  if (spacing == null) return current;
  return negative ? -spacing : spacing;
}

double? _gap(List<String> classes) {
  for (final c in classes.reversed) {
    if (c.startsWith('gap-')) {
      final spacing = _spacingUnit(c.substring(4));
      if (spacing != null) return spacing;
    }
  }
  return null;
}

String? _alignItems(List<String> classes) {
  for (final c in classes.reversed) {
    switch (c) {
      case 'items-center':
        return 'center';
      case 'items-end':
        return 'end';
      case 'items-start':
        return 'start';
      case 'items-stretch':
        return 'stretch';
    }
  }
  return null;
}

String? _justify(List<String> classes) {
  for (final c in classes.reversed) {
    switch (c) {
      case 'justify-center':
        return 'center';
      case 'justify-end':
        return 'end';
      case 'justify-between':
        return 'between';
      case 'justify-around':
        return 'around';
      case 'justify-evenly':
        return 'evenly';
      case 'justify-start':
        return 'start';
    }
  }
  return null;
}

const Map<String, double> _fontSizes = {
  'text-xs': 12,
  'text-sm': 14,
  'text-base': 16,
  'text-lg': 18,
  'text-xl': 20,
  'text-2xl': 24,
  'text-3xl': 30,
  'text-4xl': 36,
  'text-5xl': 48,
  'text-6xl': 60,
  'text-7xl': 72,
  'text-8xl': 96,
  'text-9xl': 128,
};

const Map<String, int> _fontWeights = {
  'font-thin': 100,
  'font-extralight': 200,
  'font-light': 300,
  'font-normal': 400,
  'font-medium': 500,
  'font-semibold': 600,
  'font-bold': 700,
  'font-extrabold': 800,
  'font-black': 900,
};

/// Build portable [ViewStyle] hints from Tailwind-ish utility classes.
/// Original class tokens are retained alongside the recognized hints.
ViewStyle? styleFromClasses(List<String> classes, {String? id}) {
  double? padding;
  double? px;
  double? py;
  double? pt;
  double? pb;
  double? pl;
  double? pr;
  double? margin;
  double? mx;
  double? my;
  double? mt;
  double? mb;
  double? ml;
  double? mr;
  double? fontSize;
  int? fontWeight;
  double? letterSpacing;
  double? lineHeight;
  String? textAlign;
  int? lineClamp;
  String? textOverflow;
  String? fg;
  String? bg;
  double? radius;
  double? opacity;
  double? borderWidth;
  String? borderColor;
  bool? italic;
  bool? underline;
  bool? strikethrough;

  for (final c in classes) {
    if (_fontSizes.containsKey(c)) {
      fontSize = _fontSizes[c];
      continue;
    }
    if (c.startsWith('text-[') && c.endsWith(']')) {
      final size = _spacingUnit(c.substring('text-'.length));
      if (size != null && size > 0) fontSize = size;
    }
    if (_fontWeights.containsKey(c)) {
      fontWeight = _fontWeights[c];
      continue;
    }
    if (c.startsWith('leading-')) {
      final token = c.substring('leading-'.length);
      final height = switch (token) {
        'none' => 1.0,
        'tight' => 1.25,
        'snug' => 1.375,
        'normal' => 1.5,
        'relaxed' => 1.625,
        'loose' => 2.0,
        'px' => 1.0,
        _ => _spacingUnit(token),
      };
      if (height != null && height > 0) lineHeight = height;
    }
    if (c.startsWith('tracking-')) {
      final token = c.substring('tracking-'.length);
      final spacing = switch (token) {
        'tighter' => -0.8,
        'tight' => -0.4,
        'normal' => 0.0,
        'wide' => 0.4,
        'wider' => 0.8,
        'widest' => 1.6,
        _ => token.startsWith('[') ? _spacingUnit(token) : null,
      };
      if (spacing != null) letterSpacing = spacing;
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
    if (c == 'truncate') textOverflow = 'truncate';
    if (c == 'text-ellipsis') textOverflow = 'ellipsis';
    if (c == 'text-clip') textOverflow = 'clip';
    if (c.startsWith('line-clamp-')) {
      final count = int.tryParse(c.substring('line-clamp-'.length), radix: 10);
      if (count != null && count > 0 && count <= 2147483647) {
        lineClamp = count;
      }
    }
    if (c == 'text-left') textAlign = 'left';
    if (c == 'text-center') textAlign = 'center';
    if (c == 'text-right') textAlign = 'right';
    if (c.startsWith('p-')) padding = _spacingUnit(c.substring(2)) ?? padding;
    if (c.startsWith('px-')) px = _spacingUnit(c.substring(3)) ?? px;
    if (c.startsWith('py-')) py = _spacingUnit(c.substring(3)) ?? py;
    if (c.startsWith('pt-')) pt = _spacingUnit(c.substring(3)) ?? pt;
    if (c.startsWith('pb-')) pb = _spacingUnit(c.substring(3)) ?? pb;
    if (c.startsWith('pl-')) pl = _spacingUnit(c.substring(3)) ?? pl;
    if (c.startsWith('pr-')) pr = _spacingUnit(c.substring(3)) ?? pr;
    margin = _marginClass(c, 'm-', margin);
    mx = _marginClass(c, 'mx-', mx);
    my = _marginClass(c, 'my-', my);
    mt = _marginClass(c, 'mt-', mt);
    mb = _marginClass(c, 'mb-', mb);
    ml = _marginClass(c, 'ml-', ml);
    mr = _marginClass(c, 'mr-', mr);
    if (c.startsWith('rounded')) {
      radius = _roundedValue(c) ?? radius;
    }
    if (c.startsWith('opacity-')) {
      final percentage = double.tryParse(c.substring('opacity-'.length));
      if (percentage != null && percentage.isFinite) {
        opacity = percentage.clamp(0, 100) / 100;
      }
    }
    if (c == 'border') borderWidth = 1;
    if (c.startsWith('border-')) {
      final token = c.substring('border-'.length);
      final width = token.startsWith('[')
          ? _spacingUnit(token)
          : num.tryParse(token)?.toDouble();
      if (width != null && width.isFinite && width >= 0) borderWidth = width;
    }
    final color = _colorClass(c);
    if (color != null) {
      if (color.$1 == 'text') fg = color.$2;
      if (color.$1 == 'bg') bg = color.$2;
      if (color.$1 == 'border') borderColor = color.$2;
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
    margin: margin,
    marginHorizontal: mx,
    marginVertical: my,
    marginTop: mt,
    marginBottom: mb,
    marginLeft: ml,
    marginRight: mr,
    fontSize: fontSize,
    fontWeight: fontWeight,
    letterSpacing: letterSpacing,
    lineHeight: lineHeight,
    textAlign: textAlign,
    lineClamp: lineClamp,
    textOverflow: textOverflow,
    foregroundColor: fg,
    backgroundColor: bg,
    cornerRadius: radius,
    opacity: opacity,
    borderWidth: borderWidth,
    borderColor: borderColor,
    italic: italic,
    underline: underline,
    strikethrough: strikethrough,
  );
  return style.isEmpty ? null : style;
}

double? _roundedValue(String c) {
  if (c.startsWith('rounded-[') && c.endsWith(']')) {
    final radius = _spacingUnit(c.substring('rounded-'.length));
    if (radius != null && radius >= 0) return radius;
    return 8; // Preserve the established malformed arbitrary-radius fallback.
  }
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
  return null;
}

/// Recognize palette and CSS hex text/background/border classes.
(String, String)? _colorClass(String c) {
  for (final prefix in const ['text-', 'bg-', 'border-']) {
    if (c.startsWith(prefix)) {
      final name = c.substring(prefix.length);
      final hex = _palette[name];
      if (hex != null) return (prefix.substring(0, prefix.length - 1), hex);
      if (name.startsWith('[#') && name.endsWith(']')) {
        final arbitrary = normalizeCssHex(name.substring(1, name.length - 1));
        if (arbitrary != null)
          return (prefix.substring(0, prefix.length - 1), arbitrary);
      }
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
