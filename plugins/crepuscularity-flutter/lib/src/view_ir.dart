/// Typed, constrained View IR model shared by the IR decoder (`ViewIr.fromJson`)
/// and the `.crepus` parser. Mirrors the `ViewNode` kinds in
/// `crates/crepuscularity-native/src/ir.rs` (serde tag `kind`, camelCase), but
/// only the SAFE, non-executing subset this renderer supports.
library;

/// The node kinds this renderer accepts. Anything outside this allowlist —
/// `webView`, `filePicker`, `dropzone`, free `input`/`picker`/`slider`,
/// `tabs`, `slot`, `embed`, … — decodes to [UnsupportedNode] and renders
/// nothing. Keeping the allowlist in one place makes the boundary auditable.
const Set<String> kAllowedKinds = {
  'text',
  'stack',
  'scroll',
  'button',
  'toggle',
  'checkbox',
  'progress',
  'timer',
  'meter',
  'sparkline',
  'badge',
  'divider',
  'spacer',
  'image',
  'if',
  'forEach',
  'list',
  'listItem',
  'link',
};

/// Caps applied to any parsed/decoded document. Oversized input is rejected
/// gracefully (the document renders as empty) rather than throwing.
class CrepusLimits {
  const CrepusLimits({
    this.maxNodes = 60,
    this.maxDepth = 8,
    this.maxSourceLength = 8000,
  });

  final int maxNodes;
  final int maxDepth;
  final int maxSourceLength;

  static const CrepusLimits defaults = CrepusLimits();
}

enum StackAxis { row, column }

StackAxis _axisFromString(Object? value) =>
    value == 'row' ? StackAxis.row : StackAxis.column;

/// Coerce a JSON scalar to a finite double. NaN and ±Infinity — reachable from
/// both raw JSON numbers and strings like `"nan"` — are rejected so no
/// non-finite value can ever reach a Flutter layout constraint.
double? _asDouble(Object? value) {
  final parsed = switch (value) {
    num() => value.toDouble(),
    String() => double.tryParse(value),
    _ => null,
  };
  if (parsed == null || !parsed.isFinite) return null;
  return parsed;
}

List<double> _doubleList(Object? value) {
  if (value is List) {
    return value.map(_asDouble).whereType<double>().toList(growable: false);
  }
  if (value is String) {
    return value
        .replaceAll('[', '')
        .replaceAll(']', '')
        .split(RegExp(r'[,\s]+'))
        .map((part) => _asDouble(part.trim()))
        .whereType<double>()
        .toList(growable: false);
  }
  return const [];
}

String? _asString(Object? value) => value is String ? value : null;

List<String> _stringList(Object? value) {
  if (value is! List) return const [];
  return value.whereType<String>().toList(growable: false);
}

bool _asBool(Object? value) => value == true;

/// Portable style hints (a focused subset of `ViewStyle` in `ir.rs`). Fields we
/// do not render are simply ignored on decode.
class ViewStyle {
  const ViewStyle({
    this.id,
    this.classes = const [],
    this.padding,
    this.paddingHorizontal,
    this.paddingVertical,
    this.paddingTop,
    this.paddingBottom,
    this.paddingLeft,
    this.paddingRight,
    this.fontSize,
    this.fontWeight,
    this.textAlign,
    this.foregroundColor,
    this.backgroundColor,
    this.cornerRadius,
    this.borderWidth,
    this.borderColor,
    this.opacity,
    this.italic,
    this.underline,
    this.strikethrough,
  });

  /// The source `#id`, preserved verbatim so hosts can bind to the element.
  final String? id;

  /// The source class tokens, preserved verbatim alongside the resolved hints.
  final List<String> classes;

  final double? padding;
  final double? paddingHorizontal;
  final double? paddingVertical;
  final double? paddingTop;
  final double? paddingBottom;
  final double? paddingLeft;
  final double? paddingRight;
  final double? fontSize;
  final int? fontWeight;
  final String? textAlign;
  final String? foregroundColor;
  final String? backgroundColor;
  final double? cornerRadius;
  final double? borderWidth;
  final String? borderColor;
  final double? opacity;
  final bool? italic;
  final bool? underline;
  final bool? strikethrough;

  bool get isEmpty =>
      id == null &&
      classes.isEmpty &&
      padding == null &&
      paddingHorizontal == null &&
      paddingVertical == null &&
      paddingTop == null &&
      paddingBottom == null &&
      paddingLeft == null &&
      paddingRight == null &&
      fontSize == null &&
      fontWeight == null &&
      textAlign == null &&
      foregroundColor == null &&
      backgroundColor == null &&
      cornerRadius == null &&
      borderWidth == null &&
      borderColor == null &&
      opacity == null &&
      italic == null &&
      underline == null &&
      strikethrough == null;

  static ViewStyle? fromJson(Object? value) {
    if (value is! Map) return null;
    final style = ViewStyle(
      id: _asString(value['id']),
      classes: _stringList(value['classes']),
      padding: _asDouble(value['padding']),
      paddingHorizontal: _asDouble(value['paddingHorizontal']),
      paddingVertical: _asDouble(value['paddingVertical']),
      paddingTop: _asDouble(value['paddingTop']),
      paddingBottom: _asDouble(value['paddingBottom']),
      paddingLeft: _asDouble(value['paddingLeft']),
      paddingRight: _asDouble(value['paddingRight']),
      fontSize: _asDouble(value['fontSize']),
      fontWeight: _asDouble(value['fontWeight'])?.round(),
      textAlign: _asString(value['textAlign']),
      foregroundColor: _asString(value['foregroundColor']),
      backgroundColor: _asString(value['backgroundColor']),
      cornerRadius: _asDouble(value['cornerRadius']),
      borderWidth: _asDouble(value['borderWidth']),
      borderColor: _asString(value['borderColor']),
      opacity: _asDouble(value['opacity']),
      italic: value['italic'] is bool ? value['italic'] as bool : null,
      underline: value['underline'] is bool ? value['underline'] as bool : null,
      strikethrough: value['strikethrough'] is bool
          ? value['strikethrough'] as bool
          : null,
    );
    return style.isEmpty ? null : style;
  }

  /// Re-encode to the wire shape. Absent fields are omitted rather than written
  /// as `null`, matching the `skip_serializing_if` behaviour on the Rust side.
  Map<String, Object?> toJson() {
    final out = <String, Object?>{};
    void put(String key, Object? value) {
      if (value != null) out[key] = value;
    }

    put('id', id);
    if (classes.isNotEmpty) out['classes'] = List<String>.of(classes);
    put('padding', padding);
    put('paddingHorizontal', paddingHorizontal);
    put('paddingVertical', paddingVertical);
    put('paddingTop', paddingTop);
    put('paddingBottom', paddingBottom);
    put('paddingLeft', paddingLeft);
    put('paddingRight', paddingRight);
    put('fontSize', fontSize);
    put('fontWeight', fontWeight);
    put('textAlign', textAlign);
    put('foregroundColor', foregroundColor);
    put('backgroundColor', backgroundColor);
    put('cornerRadius', cornerRadius);
    put('borderWidth', borderWidth);
    put('borderColor', borderColor);
    put('opacity', opacity);
    put('italic', italic);
    put('underline', underline);
    put('strikethrough', strikethrough);
    return out;
  }
}

/// Base class for every constrained node.
sealed class ViewNode {
  const ViewNode({this.style});

  final ViewStyle? style;

  /// Decode a single node from a JSON map. Unknown or disallowed `kind`s
  /// resolve to [UnsupportedNode] — never an exception. This is the model-layer
  /// half of the allowlist (the renderer enforces it a second time).
  static ViewNode fromJson(Object? raw) {
    if (raw is! Map) return const UnsupportedNode('<non-object>');
    final kind = raw['kind'];
    if (kind is! String || !kAllowedKinds.contains(kind)) {
      return UnsupportedNode(kind is String ? kind : '<no-kind>');
    }
    final style = ViewStyle.fromJson(raw['style']);
    switch (kind) {
      case 'text':
        return TextNode(
          content: _asString(raw['content']) ?? '',
          bind: _asString(raw['bind']),
          style: style,
        );
      case 'stack':
        return StackNode(
          axis: _axisFromString(raw['axis']),
          spacing: _asDouble(raw['spacing']),
          alignItems: _asString(raw['alignItems']),
          justifyContent: _asString(raw['justifyContent']),
          onLongPress: _asString(raw['onLongPress']),
          children: _childList(raw['children']),
          style: style,
        );
      case 'scroll':
        return ScrollNode(
          axis: _axisFromString(raw['axis']),
          children: _childList(raw['children']),
          style: style,
        );
      case 'button':
        return ButtonNode(
          label: _asString(raw['label']) ?? '',
          onClick: _asString(raw['onClick']),
          onLongPress: _asString(raw['onLongPress']),
          style: style,
        );
      case 'toggle':
        return ToggleNode(
          label: _asString(raw['label']) ?? '',
          bind: _asString(raw['bind']),
          checked: _asBool(raw['checked']),
          onChange: _asString(raw['onChange']),
          onLongPress: _asString(raw['onLongPress']),
          style: style,
        );
      case 'checkbox':
        return CheckboxNode(
          label: _asString(raw['label']) ?? '',
          bind: _asString(raw['bind']),
          checked: _asBool(raw['checked']),
          onChange: _asString(raw['onChange']),
          style: style,
        );
      case 'progress':
        return ProgressNode(
          label: _asString(raw['label']),
          value: _asDouble(raw['value']) ?? 0,
          max: _asDouble(raw['max']) ?? 100,
          style: style,
        );
      case 'timer':
        return TimerNode(
          label: _asString(raw['label']),
          duration: parseCrepusDuration(_asString(raw['duration'])),
          autostart: _asBool(raw['autostart']),
          countUp: _asBool(raw['countUp']),
          style: style,
        );
      case 'meter':
        return MeterNode(
          label: _asString(raw['label']),
          value: _asDouble(raw['value']) ?? 0,
          min: _asDouble(raw['min']) ?? 0,
          max: _asDouble(raw['max']) ?? 100,
          style: style,
        );
      case 'sparkline':
        return SparklineNode(
          values: _doubleList(raw['values']),
          color: _asString(raw['color']),
          variant: _asString(raw['variant']),
          style: style,
        );
      case 'badge':
        return BadgeNode(
          label: _asString(raw['label']) ?? '',
          tone: _asString(raw['tone']),
          style: style,
        );
      case 'divider':
        return DividerNode(axis: _axisFromString(raw['axis']), style: style);
      case 'spacer':
        return SpacerNode(size: _asDouble(raw['size']), style: style);
      case 'image':
        return ImageNode(
          src: _asString(raw['src']) ?? '',
          alt: _asString(raw['alt']),
          onLongPress: _asString(raw['onLongPress']),
          style: style,
        );
      case 'if':
        return IfNode(
          condition: _asString(raw['condition']) ?? '',
          thenChildren: _childList(raw['thenChildren']),
          elseChildren: raw['elseChildren'] == null
              ? null
              : _childList(raw['elseChildren']),
          style: style,
        );
      case 'forEach':
        return ForEachNode(
          bind: _asString(raw['bind']) ?? '',
          itemName: _asString(raw['itemName']) ?? 'item',
          itemBody: _childList(raw['itemBody']),
          style: style,
        );
      case 'list':
        return ListNode(
          ordered: _asBool(raw['ordered']),
          children: _childList(raw['children']),
          style: style,
        );
      case 'link':
        return LinkNode(
          href: _asString(raw['href']) ?? '',
          target: _asString(raw['target']),
          rel: _asString(raw['rel']),
          children: _childList(raw['children']),
          style: style,
        );
      case 'listItem':
        return ListItemNode(
          onClick: _asString(raw['onClick']),
          onLongPress: _asString(raw['onLongPress']),
          children: _childList(raw['children']),
          style: style,
        );
    }
    return UnsupportedNode(kind);
  }

  static List<ViewNode> _childList(Object? raw) {
    if (raw is! List) return const [];
    return raw.map(ViewNode.fromJson).toList(growable: false);
  }
}

class TextNode extends ViewNode {
  const TextNode({required this.content, this.bind, super.style});
  final String content;
  final String? bind;
}

class StackNode extends ViewNode {
  const StackNode({
    required this.axis,
    this.spacing,
    this.alignItems,
    this.justifyContent,
    this.onLongPress,
    required this.children,
    super.style,
  });
  final StackAxis axis;
  final double? spacing;
  final String? alignItems;
  final String? justifyContent;
  final String? onLongPress;
  final List<ViewNode> children;
}

class ScrollNode extends ViewNode {
  const ScrollNode({required this.axis, required this.children, super.style});
  final StackAxis axis;
  final List<ViewNode> children;
}

class ButtonNode extends ViewNode {
  const ButtonNode({
    required this.label,
    this.onClick,
    this.onLongPress,
    super.style,
  });
  final String label;
  final String? onClick;
  final String? onLongPress;
}

class ToggleNode extends ViewNode {
  const ToggleNode({
    required this.label,
    this.bind,
    required this.checked,
    this.onChange,
    this.onLongPress,
    super.style,
  });
  final String label;
  final String? bind;
  final bool checked;
  final String? onChange;
  final String? onLongPress;
}

class CheckboxNode extends ViewNode {
  const CheckboxNode({
    required this.label,
    this.bind,
    required this.checked,
    this.onChange,
    super.style,
  });
  final String label;
  final String? bind;
  final bool checked;
  final String? onChange;
}

class ProgressNode extends ViewNode {
  const ProgressNode({
    this.label,
    required this.value,
    required this.max,
    super.style,
  });
  final String? label;
  final double value;
  final double max;
}

/// A clock the renderer runs, rather than a number the author wrote once.
///
/// Every other node here is a still frame: `progress value=4` is four percent
/// forever. A timer owns elapsed time, so the bar it draws actually moves and
/// the remaining figure actually counts down. Duration accepts `90s`, `25m`,
/// `1h`, or a bare number of seconds.
class TimerNode extends ViewNode {
  const TimerNode({
    this.label,
    required this.duration,
    this.autostart = false,
    this.countUp = false,
    super.style,
  });
  final String? label;
  final Duration duration;

  /// Whether it starts running as soon as it appears, rather than waiting to
  /// be started.
  final bool autostart;

  /// Count up from zero instead of down to it. Stopwatch rather than egg timer.
  final bool countUp;
}

class MeterNode extends ViewNode {
  const MeterNode({
    this.label,
    required this.value,
    required this.min,
    required this.max,
    super.style,
  });
  final String? label;
  final double value;
  final double min;
  final double max;
}

class SparklineNode extends ViewNode {
  const SparklineNode({
    required this.values,
    this.color,
    this.variant,
    super.style,
  });
  final List<double> values;
  final String? color;
  final String? variant;
}

class BadgeNode extends ViewNode {
  const BadgeNode({required this.label, this.tone, super.style});
  final String label;
  final String? tone;
}

class DividerNode extends ViewNode {
  const DividerNode({required this.axis, super.style});
  final StackAxis axis;
}

class SpacerNode extends ViewNode {
  const SpacerNode({this.size, super.style});
  final double? size;
}

class ImageNode extends ViewNode {
  const ImageNode({required this.src, this.alt, this.onLongPress, super.style});
  final String src;
  final String? alt;
  final String? onLongPress;
}

class IfNode extends ViewNode {
  const IfNode({
    required this.condition,
    required this.thenChildren,
    this.elseChildren,
    super.style,
  });
  final String condition;
  final List<ViewNode> thenChildren;
  final List<ViewNode>? elseChildren;
}

class ForEachNode extends ViewNode {
  const ForEachNode({
    required this.bind,
    required this.itemName,
    required this.itemBody,
    super.style,
  });
  final String bind;
  final String itemName;
  final List<ViewNode> itemBody;
}

class ListNode extends ViewNode {
  const ListNode({required this.ordered, required this.children, super.style});
  final bool ordered;
  final List<ViewNode> children;
}

class ListItemNode extends ViewNode {
  const ListItemNode({
    this.onClick,
    this.onLongPress,
    required this.children,
    super.style,
  });
  final String? onClick;
  final String? onLongPress;
  final List<ViewNode> children;
}

/// A container whose subtree is tappable, carrying the destination `href`.
/// The href is handed to the host through the action callback; this renderer
/// never opens a URL itself.
class LinkNode extends ViewNode {
  const LinkNode({
    required this.href,
    this.target,
    this.rel,
    required this.children,
    super.style,
  });
  final String href;
  final String? target;
  final String? rel;
  final List<ViewNode> children;
}

/// A node whose kind is unknown or outside [kAllowedKinds]. Renders nothing.
class UnsupportedNode extends ViewNode {
  const UnsupportedNode(this.kind);
  final String kind;
}

/// A decoded document: `{ version, root: [...] }`.
class ViewIr {
  const ViewIr({required this.version, required this.root});

  final int version;
  final List<ViewNode> root;

  /// Decode from the wire map. Enforces [CrepusLimits]; oversized documents
  /// yield an empty root rather than throwing.
  static ViewIr fromJson(
    Map<Object?, Object?> json, {
    CrepusLimits limits = CrepusLimits.defaults,
  }) {
    final version = _asDouble(json['version'])?.round() ?? 0;
    final rawRoot = json['root'];
    final root = rawRoot is List
        ? rawRoot.map(ViewNode.fromJson).toList(growable: false)
        : const <ViewNode>[];
    if (crepusExceedsLimits(root, limits)) {
      return ViewIr(version: version, root: const []);
    }
    return ViewIr(version: version, root: root);
  }
}

/// Whether a node list breaks the node-count or depth caps. Shared by the IR
/// decoder and the `.crepus` parser so both enforce the same bounds.
bool crepusExceedsLimits(List<ViewNode> nodes, CrepusLimits limits) {
  var count = 0;
  bool walk(List<ViewNode> list, int depth) {
    if (depth > limits.maxDepth) return true;
    for (final node in list) {
      count += 1;
      if (count > limits.maxNodes) return true;
      final children = childrenOf(node);
      if (children.isNotEmpty && walk(children, depth + 1)) return true;
    }
    return false;
  }

  return walk(nodes, 1);
}

/// The structural children of a node (empty for leaves). Used for caps + walks.
List<ViewNode> childrenOf(ViewNode node) => switch (node) {
  StackNode(:final children) => children,
  ScrollNode(:final children) => children,
  ListNode(:final children) => children,
  ListItemNode(:final children) => children,
  LinkNode(:final children) => children,
  IfNode(:final thenChildren, :final elseChildren) => [
    ...thenChildren,
    ...?elseChildren,
  ],
  ForEachNode(:final itemBody) => itemBody,
  _ => const [],
};

/// Reads `90s`, `25m`, `1h`, `500ms`, or a bare number of seconds. Anything
/// unparseable is a zero duration rather than a throw: a malformed artifact
/// should render a dead timer, not take the message down with it.
Duration parseCrepusDuration(String? raw) {
  final text = raw?.trim().toLowerCase();
  if (text == null || text.isEmpty) return Duration.zero;
  final match = RegExp(r'^([0-9]*\.?[0-9]+)\s*(ms|s|m|h)?$').firstMatch(text);
  if (match == null) return Duration.zero;
  final amount = double.tryParse(match.group(1)!);
  if (amount == null || amount <= 0) return Duration.zero;
  final micros = switch (match.group(2)) {
    'ms' => amount * 1000,
    'm' => amount * 60 * 1000000,
    'h' => amount * 3600 * 1000000,
    _ => amount * 1000000,
  };
  return Duration(microseconds: micros.round());
}
