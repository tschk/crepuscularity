/// Renders the constrained [ViewIr] model to Flutter widgets.
///
/// This package is deliberately GENERIC and action-agnostic: `on_click` /
/// `on_change` / `on_long_press` strings are surfaced verbatim to the host via
/// [CrepusView.onAction]. The package never interprets, maps, or `eval`s an
/// action — the host owns that policy. Unknown/disallowed node kinds render
/// nothing.
library;

import 'dart:async';

import 'package:flutter/material.dart';

import 'package:crepuscularity_components/crepuscularity_components.dart';

import 'crepus_parser.dart';
import 'view_ir.dart';

/// Host callback for a surfaced action string. The package does not interpret
/// the string in any way; the host decides what (if anything) it means.
typedef CrepusActionCallback = void Function(String action);

/// Width used for a progress/meter bar whose parent supplies unbounded width.
const double _unboundedBarWidth = 120;

/// Upper bound applied to every model-authored layout magnitude (padding,
/// spacing, spacer size, corner radius, border width). It is far larger than
/// any real viewport, so legitimate documents are untouched, but it stops an
/// absurd finite value like `1e9` from producing an unusable, overflowing
/// render. Non-finite values are already rejected at decode time.
const double _maxLayoutExtent = 4000;

/// Largest font size honoured, for the same reason as [_maxLayoutExtent].
const double _maxFontSize = 400;

double _extent(double value) =>
    value <= 0 ? 0 : (value > _maxLayoutExtent ? _maxLayoutExtent : value);

/// Theming knobs. Defaults are derived from the ambient [ThemeData] so the
/// renderer looks reasonable with zero configuration; a host (e.g. omi's hub)
/// can override to match its palette.
class CrepusTheme {
  const CrepusTheme({
    this.textColor,
    this.mutedColor,
    this.accentColor,
    this.surfaceColor,
    this.borderColor,
    this.baseFontSize = 14,
  });

  final Color? textColor;
  final Color? mutedColor;
  final Color? accentColor;
  final Color? surfaceColor;
  final Color? borderColor;
  final double baseFontSize;

  CrepusTheme _resolved(BuildContext context) {
    final theme = Theme.of(context);
    final onSurface = theme.colorScheme.onSurface;
    return CrepusTheme(
      textColor: textColor ?? onSurface,
      mutedColor: mutedColor ?? onSurface.withValues(alpha: 0.6),
      accentColor: accentColor ?? theme.colorScheme.primary,
      surfaceColor: surfaceColor ?? theme.colorScheme.surface,
      borderColor: borderColor ?? theme.dividerColor,
      baseFontSize: baseFontSize,
    );
  }
}

/// A widget that renders Crepuscularity View IR or `.crepus` source.
class CrepusView extends StatelessWidget {
  /// Render from decoded View IR JSON (`{ version, root: [...] }`).
  CrepusView.fromIr(
    Map<Object?, Object?> ir, {
    super.key,
    this.onAction,
    this.theme = const CrepusTheme(),
    this.data = const {},
    this.allowNetworkImages = false,
    CrepusLimits limits = CrepusLimits.defaults,
  }) : _ir = ViewIr.fromJson(ir, limits: limits);

  /// Render from `.crepus` source (parsed in pure Dart — no Rust round-trip).
  CrepusView.fromSource(
    String source, {
    super.key,
    this.onAction,
    this.theme = const CrepusTheme(),
    this.data = const {},
    this.allowNetworkImages = false,
    CrepusLimits limits = CrepusLimits.defaults,
  }) : _ir = viewIrFromSource(source, limits: limits);

  final ViewIr _ir;
  final CrepusActionCallback? onAction;
  final CrepusTheme theme;

  /// Runtime scope for `{...}` interpolation and `if`/`forEach` bindings.
  final Map<String, Object?> data;

  final bool allowNetworkImages;

  /// The decoded document (exposed for tests / introspection).
  ViewIr get ir => _ir;

  @override
  Widget build(BuildContext context) {
    final resolved = theme._resolved(context);
    final renderer = _Renderer(resolved, onAction, allowNetworkImages);
    final children = renderer.renderList(_ir.root, data);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: children,
    );
  }
}

class _Renderer {
  _Renderer(this.theme, this.onAction, this.allowNetworkImages);

  final CrepusTheme theme;
  final CrepusActionCallback? onAction;
  final bool allowNetworkImages;

  List<Widget> renderList(List<ViewNode> nodes, Map<String, Object?> scope) {
    final widgets = <Widget>[];
    for (final node in nodes) {
      final rendered = render(node, scope);
      if (rendered != null) widgets.add(rendered);
    }
    return widgets;
  }

  Widget? render(ViewNode node, Map<String, Object?> scope) {
    final widget = switch (node) {
      TextNode() => _text(node, scope),
      StackNode() => _stack(node, scope),
      ScrollNode() => _scroll(node, scope),
      ButtonNode() => _button(node),
      ToggleNode() => _toggle(node),
      CheckboxNode() => _checkbox(node),
      ProgressNode() => _progress(node),
      TimerNode() => _timer(node),
      MeterNode() => _meter(node),
      SparklineNode() => _sparkline(node),
      BadgeNode() => _badge(node),
      DividerNode() => _divider(node),
      SpacerNode() => _spacer(node),
      ImageNode() => _image(node),
      ListNode() => _list(node, scope),
      ListItemNode() => _listItem(node, scope),
      LinkNode() => _link(node, scope),
      IfNode() => _ifNode(node, scope),
      ForEachNode() => _forEach(node, scope),
      // Unknown/disallowed kinds render nothing (a faint chip in debug only).
      UnsupportedNode() => _unsupported(node),
    };
    if (widget == null) return null;
    return _applyStyle(widget, node.style);
  }

  // ── Leaf + container renderers ────────────────────────────────────────────

  Widget? _text(TextNode node, Map<String, Object?> scope) {
    var content = node.content;
    if (node.bind != null && node.bind!.isNotEmpty) {
      final bound = _lookup(node.bind!, scope);
      if (bound != null) content = '$bound';
    }
    content = _interpolate(content, scope);
    if (content.isEmpty) return null;
    final style = node.style;
    return Text(
      content,
      style: TextStyle(
        color: _color(style?.foregroundColor) ?? theme.textColor,
        fontSize: _fontSize(style?.fontSize),
        fontWeight: _weight(style?.fontWeight),
        fontStyle: style?.italic == true ? FontStyle.italic : null,
        decoration: _decoration(style),
        height: 1.35,
      ),
      textAlign: _textAlign(style?.textAlign),
    );
  }

  Widget _stack(StackNode node, Map<String, Object?> scope) {
    final children = renderList(node.children, scope);
    final spaced = _withSpacing(children, node.spacing, node.axis);
    final widget = node.axis == StackAxis.row
        ? Row(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: _crossAxis(node.alignItems),
            mainAxisAlignment: _mainAxis(node.justifyContent),
            children: spaced,
          )
        : Column(
            crossAxisAlignment: _crossAxisCol(node.alignItems),
            mainAxisSize: MainAxisSize.min,
            mainAxisAlignment: _mainAxis(node.justifyContent),
            children: spaced,
          );
    return _withLongPress(widget, node.onLongPress);
  }

  Widget _scroll(ScrollNode node, Map<String, Object?> scope) {
    final children = renderList(node.children, scope);
    return SingleChildScrollView(
      scrollDirection: node.axis == StackAxis.row
          ? Axis.horizontal
          : Axis.vertical,
      child: node.axis == StackAxis.row
          ? Row(mainAxisSize: MainAxisSize.min, children: children)
          : Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: children,
            ),
    );
  }

  Widget _button(ButtonNode node) {
    final child = Text(node.label);
    final button = TextButton(
      onPressed: node.onClick == null ? null : () => _dispatch(node.onClick),
      style: TextButton.styleFrom(
        foregroundColor: theme.accentColor,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      ),
      child: child,
    );
    return _withLongPress(button, node.onLongPress);
  }

  Widget _toggle(ToggleNode node) {
    final control = Switch(
      value: node.checked,
      onChanged: node.onChange == null ? null : (_) => _dispatch(node.onChange),
    );
    final row = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (node.label.isNotEmpty)
          Flexible(child: Text(node.label, style: _bodyStyle())),
        control,
      ],
    );
    return _withLongPress(row, node.onLongPress);
  }

  Widget _checkbox(CheckboxNode node) {
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Checkbox(
          value: node.checked,
          onChanged: node.onChange == null
              ? null
              : (_) => _dispatch(node.onChange),
        ),
        if (node.label.isNotEmpty)
          Flexible(child: Text(node.label, style: _bodyStyle())),
      ],
    );
  }

  Widget _progress(ProgressNode node) {
    final fraction = node.max <= 0 ? 0.0 : (node.value / node.max).clamp(0, 1);
    final percent = (fraction * 100).round();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (node.label != null && node.label!.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(bottom: 4),
            child: Text(node.label!, style: _mutedStyle()),
          ),
        _labeledBar(fraction.toDouble(), '$percent%'),
      ],
    );
  }

  Widget _timer(TimerNode node) => CrepusTimer(
    node: node,
    barBuilder: _labeledBar,
    labelStyle: _mutedStyle(),
  );

  Widget _meter(MeterNode node) {
    final span = node.max - node.min;
    final fraction = span <= 0
        ? 0.0
        : ((node.value - node.min) / span).clamp(0, 1);
    final percent = (fraction * 100).round();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (node.label != null && node.label!.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(bottom: 4),
            child: Text(node.label!, style: _mutedStyle()),
          ),
        _labeledBar(fraction.toDouble(), '$percent%'),
      ],
    );
  }

  Widget? _sparkline(SparklineNode node) {
    if (node.values.length < 2) return null;
    return DitherSparkline(
      values: node.values,
      color: ditherColorFromName(node.color),
      variant: ditherVariantFromName(node.variant),
      height: 64,
    );
  }

  /// Progress/meter bar plus a percentage label. Avoids [Expanded] under
  /// unbounded horizontal constraints (horizontal stack/scroll parents).
  Widget _labeledBar(double fraction, String label) => LayoutBuilder(
    builder: (context, constraints) {
      final bar = _bar(fraction);
      final percentText = Padding(
        padding: const EdgeInsets.only(left: 8),
        child: Text(label, style: _mutedStyle()),
      );
      if (constraints.hasBoundedWidth) {
        return Row(
          children: [
            Expanded(child: bar),
            percentText,
          ],
        );
      }
      return Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(width: _unboundedBarWidth, child: bar),
          percentText,
        ],
      );
    },
  );

  /// The shared progress/meter bar. [LinearProgressIndicator] has no intrinsic
  /// width, so inside a horizontal `stack`/`scroll` — which hands its children
  /// unbounded width — it would size to infinity and fail layout. Fall back to
  /// a fixed width whenever the incoming constraints are unbounded.
  Widget _bar(double fraction) => LayoutBuilder(
    builder: (context, constraints) {
      final bar = ClipRRect(
        borderRadius: BorderRadius.circular(4),
        child: LinearProgressIndicator(
          value: fraction,
          minHeight: 6,
          backgroundColor: theme.borderColor,
          valueColor: AlwaysStoppedAnimation(theme.accentColor!),
        ),
      );
      if (constraints.hasBoundedWidth) return bar;
      return SizedBox(width: _unboundedBarWidth, child: bar);
    },
  );

  Widget _badge(BadgeNode node) {
    final tone = _toneColor(node.tone);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 3),
      decoration: BoxDecoration(
        color: tone.withValues(alpha: 0.14),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        node.label,
        style: TextStyle(
          fontSize: 11,
          fontWeight: FontWeight.w600,
          color: tone,
        ),
      ),
    );
  }

  Widget _divider(DividerNode node) => node.axis == StackAxis.row
      ? SizedBox(
          height: 16,
          child: VerticalDivider(width: 1, color: theme.borderColor),
        )
      : Divider(height: 1, color: theme.borderColor);

  Widget _spacer(SpacerNode node) {
    final size = _extent(node.size ?? 8);
    return SizedBox(width: size, height: size);
  }

  Widget _image(ImageNode node) {
    final uri = Uri.tryParse(node.src);
    final isNetwork =
        uri != null && (uri.scheme == 'http' || uri.scheme == 'https');
    final placeholder = _imagePlaceholder(node);
    final image = isNetwork && allowNetworkImages
        ? Image.network(
            node.src,
            fit: BoxFit.cover,
            errorBuilder: (_, _, _) => placeholder,
          )
        : placeholder;
    return _withLongPress(image, node.onLongPress);
  }

  Widget _imagePlaceholder(ImageNode node) => Container(
    padding: const EdgeInsets.all(8),
    color: theme.surfaceColor,
    child: Text(node.alt ?? '', style: _mutedStyle()),
  );

  Widget _list(ListNode node, Map<String, Object?> scope) {
    final items = <Widget>[];
    var index = 1;
    for (final child in node.children) {
      final marker = node.ordered ? '$index.' : '•';
      final rendered = render(child, scope);
      if (rendered == null) continue;
      items.add(
        Padding(
          padding: const EdgeInsets.symmetric(vertical: 2),
          // A `Flexible` child cannot lay out under unbounded width, which is
          // exactly what a horizontal `stack`/`scroll` parent supplies, so the
          // marker row degrades to a rigid, intrinsically-sized row there.
          child: LayoutBuilder(
            builder: (context, constraints) {
              final bounded = constraints.hasBoundedWidth;
              return Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: bounded ? MainAxisSize.max : MainAxisSize.min,
                children: [
                  Padding(
                    padding: const EdgeInsets.only(right: 6),
                    child: Text(marker, style: _mutedStyle()),
                  ),
                  if (bounded) Flexible(child: rendered) else rendered,
                ],
              );
            },
          ),
        ),
      );
      index += 1;
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: items,
    );
  }

  Widget? _listItem(ListItemNode node, Map<String, Object?> scope) {
    final children = renderList(node.children, scope);
    if (children.isEmpty) return null;
    final widget = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: children,
    );
    return _withTap(_withLongPress(widget, node.onLongPress), node.onClick);
  }

  Widget? _link(LinkNode node, Map<String, Object?> scope) {
    final children = renderList(node.children, scope);
    if (children.isEmpty) return null;
    final widget = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: children,
    );
    return _withTap(widget, node.href);
  }

  Widget? _ifNode(IfNode node, Map<String, Object?> scope) {
    final branch = _evalCondition(node.condition, scope)
        ? node.thenChildren
        : node.elseChildren;
    if (branch == null || branch.isEmpty) return null;
    final children = renderList(branch, scope);
    if (children.isEmpty) return null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: children,
    );
  }

  Widget? _forEach(ForEachNode node, Map<String, Object?> scope) {
    final source = scope[node.bind];
    if (source is! List) return null;
    final children = <Widget>[];
    for (final item in source) {
      final itemScope = {...scope, node.itemName: item};
      children.addAll(renderList(node.itemBody, itemScope));
    }
    if (children.isEmpty) return null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: children,
    );
  }

  Widget? _unsupported(UnsupportedNode node) {
    // In debug builds only, surface a faint chip so authors notice a disallowed
    // kind; release builds render nothing at all.
    var debug = false;
    assert(() {
      debug = true;
      return true;
    }());
    if (!debug) return null;
    return Opacity(
      opacity: 0.4,
      child: Text('unsupported: ${node.kind}', style: _mutedStyle()),
    );
  }

  // ── The single dispatch seam to the host ──────────────────────────────────

  void _dispatch(String? action) {
    if (action == null || action.isEmpty) return;
    onAction?.call(action);
  }

  Widget _withLongPress(Widget child, String? action) {
    if (action == null || action.isEmpty) return child;
    return GestureDetector(onLongPress: () => _dispatch(action), child: child);
  }

  Widget _withTap(Widget child, String? action) {
    if (action == null || action.isEmpty) return child;
    return GestureDetector(onTap: () => _dispatch(action), child: child);
  }

  // ── Style + helpers ───────────────────────────────────────────────────────

  Widget _applyStyle(Widget child, ViewStyle? style) {
    if (style == null) return child;
    var result = child;
    final padding = _padding(style);
    if (padding != null) result = Padding(padding: padding, child: result);
    final bg = _color(style.backgroundColor);
    final borderColor = _color(style.borderColor);
    final radius = style.cornerRadius;
    final safeRadius = radius != null && radius.isFinite && radius > 0
        ? _extent(radius)
        : null;
    if (bg != null || safeRadius != null || borderColor != null) {
      final borderWidth = style.borderWidth;
      result = DecoratedBox(
        decoration: BoxDecoration(
          color: bg,
          borderRadius: safeRadius == null
              ? null
              : BorderRadius.circular(safeRadius),
          border: borderColor == null
              ? null
              : Border.all(
                  color: borderColor,
                  width:
                      borderWidth != null &&
                          borderWidth.isFinite &&
                          borderWidth >= 0
                      ? _extent(borderWidth)
                      : 1,
                ),
        ),
        child: result,
      );
    }
    if (style.opacity != null) {
      result = Opacity(opacity: style.opacity!.clamp(0, 1), child: result);
    }
    return result;
  }

  EdgeInsets? _padding(ViewStyle s) {
    if (s.padding == null &&
        s.paddingHorizontal == null &&
        s.paddingVertical == null &&
        s.paddingTop == null &&
        s.paddingBottom == null &&
        s.paddingLeft == null &&
        s.paddingRight == null) {
      return null;
    }
    // `EdgeInsets` asserts non-negative insets, so a model-authored negative
    // padding would otherwise crash layout. Clamp instead of throwing.
    double side(double? a, double? b, double? c) {
      return _extent(a ?? b ?? c ?? 0);
    }

    return EdgeInsets.only(
      top: side(s.paddingTop, s.paddingVertical, s.padding),
      bottom: side(s.paddingBottom, s.paddingVertical, s.padding),
      left: side(s.paddingLeft, s.paddingHorizontal, s.padding),
      right: side(s.paddingRight, s.paddingHorizontal, s.padding),
    );
  }

  List<Widget> _withSpacing(
    List<Widget> children,
    double? spacing,
    StackAxis axis,
  ) {
    if (spacing == null || spacing <= 0 || children.length < 2) return children;
    final size = _extent(spacing);
    final gap = axis == StackAxis.row
        ? SizedBox(width: size)
        : SizedBox(height: size);
    final out = <Widget>[];
    for (var i = 0; i < children.length; i++) {
      if (i > 0) out.add(gap);
      out.add(children[i]);
    }
    return out;
  }

  double _fontSize(double? size) {
    if (size == null) return theme.baseFontSize;
    if (size <= 0) return theme.baseFontSize;
    return size > _maxFontSize ? _maxFontSize : size;
  }

  TextStyle _bodyStyle() =>
      TextStyle(color: theme.textColor, fontSize: theme.baseFontSize);

  TextStyle _mutedStyle() =>
      TextStyle(color: theme.mutedColor, fontSize: theme.baseFontSize - 2);

  Color _toneColor(String? tone) {
    switch (tone) {
      case 'positive':
      case 'success':
        return const Color(0xff22c55e);
      case 'warning':
        return const Color(0xfff59e0b);
      case 'negative':
      case 'danger':
        return const Color(0xffef4444);
      case 'info':
        return theme.accentColor!;
    }
    return theme.mutedColor!;
  }

  FontWeight _weight(int? weight) {
    if (weight == null) return FontWeight.w400;
    if (weight >= 800) return FontWeight.w800;
    if (weight >= 700) return FontWeight.w700;
    if (weight >= 600) return FontWeight.w600;
    if (weight >= 500) return FontWeight.w500;
    if (weight >= 400) return FontWeight.w400;
    return FontWeight.w300;
  }

  TextDecoration? _decoration(ViewStyle? style) {
    final decorations = <TextDecoration>[
      if (style?.underline == true) TextDecoration.underline,
      if (style?.strikethrough == true) TextDecoration.lineThrough,
    ];
    if (decorations.isEmpty) return null;
    return TextDecoration.combine(decorations);
  }

  TextAlign? _textAlign(String? value) => switch (value) {
    'center' => TextAlign.center,
    'right' || 'trailing' => TextAlign.right,
    'left' || 'leading' => TextAlign.left,
    _ => null,
  };

  /// Row cross-axis. `stretch` is deliberately NOT honored here: a stretched
  /// [Row] demands a bounded height, and the renderer's own outer [Column] is
  /// `mainAxisSize.min`, so `stack row items-stretch` would throw an unbounded
  /// -constraint assertion on otherwise valid model-authored input.
  CrossAxisAlignment _crossAxis(String? align) => switch (align) {
    'center' => CrossAxisAlignment.center,
    'end' => CrossAxisAlignment.end,
    _ => CrossAxisAlignment.start,
  };

  CrossAxisAlignment _crossAxisCol(String? align) => switch (align) {
    'center' => CrossAxisAlignment.center,
    'end' => CrossAxisAlignment.end,
    'stretch' => CrossAxisAlignment.stretch,
    _ => CrossAxisAlignment.start,
  };

  MainAxisAlignment _mainAxis(String? justify) => switch (justify) {
    'center' => MainAxisAlignment.center,
    'end' => MainAxisAlignment.end,
    'between' => MainAxisAlignment.spaceBetween,
    'around' => MainAxisAlignment.spaceAround,
    _ => MainAxisAlignment.start,
  };

  Color? _color(String? hex) {
    if (hex == null) return null;
    var t = hex.trim();
    if (t.startsWith('#')) t = t.substring(1);
    if (t.length == 6) t = 'ff$t';
    if (t.length == 8) {
      // Hex is RRGGBBAA in some pipelines and AARRGGBB here; ir.rs emits
      // #RRGGBB(AA). Normalize #RRGGBBAA → 0xAARRGGBB.
      final value = int.tryParse(t, radix: 16);
      if (value == null) return null;
      // If the source had 8 digits (RRGGBBAA), rotate alpha to the front.
      if (hex.replaceAll('#', '').length == 8) {
        final rgb = value >> 8;
        final a = value & 0xff;
        return Color((a << 24) | rgb);
      }
      return Color(value);
    }
    return null;
  }

  /// Substitute `{path}` tokens from [scope] (dotted paths into maps). Missing
  /// keys resolve to empty string. NOT an expression evaluator.
  String _interpolate(String content, Map<String, Object?> scope) {
    if (!content.contains('{')) return content;
    return content.replaceAllMapped(RegExp(r'\{([^}]*)\}'), (match) {
      final path = match.group(1)!.trim();
      final value = _lookup(path, scope);
      return value == null ? '' : '$value';
    });
  }

  Object? _lookup(String path, Map<String, Object?> scope) {
    final parts = path.split('.');
    Object? current = scope[parts.first];
    for (final part in parts.skip(1)) {
      if (current is Map) {
        current = current[part];
      } else {
        return null;
      }
    }
    return current;
  }

  /// Evaluate a constrained `if` condition WITHOUT eval. Supports: a bare
  /// operand (a literal, or a scope lookup, tested for truthiness), `!operand`,
  /// and `left == "value"` / `left != "value"`.
  bool _evalCondition(String condition, Map<String, Object?> scope) {
    var expr = condition.trim();
    if (expr.isEmpty) return false;
    for (final op in const ['==', '!=']) {
      final idx = expr.indexOf(op);
      if (idx > 0) {
        final left = _operand(expr.substring(0, idx).trim(), scope);
        final right = _operand(expr.substring(idx + op.length).trim(), scope);
        final equal = '$left' == '$right';
        return op == '==' ? equal : !equal;
      }
    }
    if (expr.startsWith('!')) {
      return !_truthy(_operand(expr.substring(1).trim(), scope));
    }
    return _truthy(_operand(expr, scope));
  }

  Object? _operand(String token, Map<String, Object?> scope) {
    if (token.startsWith('"') && token.endsWith('"') && token.length >= 2) {
      return token.substring(1, token.length - 1);
    }
    if (token == 'true') return true;
    if (token == 'false') return false;
    final number = num.tryParse(token);
    if (number != null) return number;
    return _lookup(token, scope);
  }

  bool _truthy(Object? value) {
    if (value == null) return false;
    if (value is bool) return value;
    if (value is num) return value != 0;
    if (value is String) return value.isNotEmpty;
    if (value is Iterable) return value.isNotEmpty;
    if (value is Map) return value.isNotEmpty;
    return true;
  }
}

/// The one node that owns a clock.
///
/// Everything else the renderer produces is a still frame built from constants
/// the author wrote. This ticks: a `Ticker`-free periodic timer moves the bar
/// and the remaining figure, and start / pause / reset are real controls rather
/// than round trips through the model. It stops itself when it reaches the end
/// and stops when the widget goes away.
class CrepusTimer extends StatefulWidget {
  const CrepusTimer({
    required this.node,
    required this.barBuilder,
    required this.labelStyle,
    super.key,
  });

  final TimerNode node;
  final Widget Function(double fraction, String label) barBuilder;
  final TextStyle? labelStyle;

  @override
  State<CrepusTimer> createState() => _CrepusTimerState();
}

class _CrepusTimerState extends State<CrepusTimer> {
  Timer? _ticker;
  Duration _elapsed = Duration.zero;
  bool _running = false;

  /// Fine enough that the seconds figure never visibly stalls, coarse enough
  /// that an idle artifact is not repainting at frame rate.
  static const _tick = Duration(milliseconds: 200);

  bool _autostarted = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    // Autostart lives here rather than initState because it consults
    // MediaQuery, which is not available until dependencies resolve.
    if (_autostarted) return;
    _autostarted = true;
    if (widget.node.autostart) _start();
  }

  @override
  void didUpdateWidget(covariant CrepusTimer old) {
    super.didUpdateWidget(old);
    // A re-rendered artifact with a different duration is a different timer.
    if (old.node.duration != widget.node.duration) _reset();
  }

  bool get _animationsOff =>
      MediaQuery.maybeOf(context)?.disableAnimations ?? false;

  void _start() {
    if (_running || widget.node.duration <= Duration.zero) return;
    if (_animationsOff) return;
    _running = true;
    _ticker?.cancel();
    _ticker = Timer.periodic(_tick, (_) {
      if (!mounted) return;
      setState(() {
        _elapsed += _tick;
        if (_elapsed >= widget.node.duration) {
          _elapsed = widget.node.duration;
          _stop();
        }
      });
    });
  }

  void _stop() {
    _ticker?.cancel();
    _ticker = null;
    _running = false;
  }

  void _reset() {
    _stop();
    if (mounted) setState(() => _elapsed = Duration.zero);
  }

  @override
  void dispose() {
    _stop();
    super.dispose();
  }

  String _clock(Duration d) {
    final total = d.inSeconds;
    final minutes = (total ~/ 60).toString().padLeft(2, '0');
    final seconds = (total % 60).toString().padLeft(2, '0');
    return '$minutes:$seconds';
  }

  @override
  Widget build(BuildContext context) {
    final total = widget.node.duration;
    final fraction = total <= Duration.zero
        ? 0.0
        : (_elapsed.inMicroseconds / total.inMicroseconds).clamp(0.0, 1.0);
    final shown = widget.node.countUp ? _elapsed : total - _elapsed;
    final done = total > Duration.zero && _elapsed >= total;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (widget.node.label case final label? when label.isNotEmpty)
          Padding(
            padding: const EdgeInsets.only(bottom: 4),
            child: Text(label, style: widget.labelStyle),
          ),
        widget.barBuilder(fraction, _clock(shown)),
        Padding(
          padding: const EdgeInsets.only(top: 4),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (!done)
                TextButton(
                  onPressed: _running ? () => setState(_stop) : _start,
                  child: Text(_running ? 'Pause' : 'Start'),
                ),
              TextButton(onPressed: _reset, child: const Text('Reset')),
              if (done)
                Padding(
                  padding: const EdgeInsets.only(left: 4),
                  child: Text('Done', style: widget.labelStyle),
                ),
            ],
          ),
        ),
      ],
    );
  }
}
