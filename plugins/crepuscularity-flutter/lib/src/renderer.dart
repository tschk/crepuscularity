/// Renders the constrained [ViewIr] model to Flutter widgets.
///
/// This package is deliberately GENERIC and action-agnostic: `on_click` /
/// `on_change` / `on_long_press` strings are surfaced verbatim to the host via
/// [CrepusView.onAction]. The package never interprets, maps, or `eval`s an
/// action — the host owns that policy. Unknown/disallowed node kinds render
/// nothing.
library;

import 'dart:async';
import 'dart:collection';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart' show defaultTargetPlatform;
import 'package:flutter/rendering.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/services.dart';
import 'color_literals.dart';
import 'unicode_case.dart';

import 'package:crepuscularity_components/crepuscularity_components.dart';

import 'crepus_parser.dart';
import 'view_ir.dart';

part 'signed_margin.dart';

/// Host callback for a surfaced action string. The package does not interpret
/// the string in any way; the host decides what (if anything) it means.
typedef CrepusActionCallback = void Function(String action);

/// Width used for a progress/meter bar whose parent supplies unbounded width.
const double _unboundedBarWidth = 120;

/// Upper bound applied to every model-authored layout magnitude (padding,
/// signed margins, spacing, spacer size, corner radius, border width). It is far larger than
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
  }) : _limits = limits,
       _ir = ViewIr.fromJson(ir, limits: limits);

  /// Render from `.crepus` source (parsed in pure Dart — no Rust round-trip).
  CrepusView.fromSource(
    String source, {
    super.key,
    this.onAction,
    this.theme = const CrepusTheme(),
    this.data = const {},
    this.allowNetworkImages = false,
    CrepusLimits limits = CrepusLimits.defaults,
  }) : _limits = limits,
       _ir = viewIrFromSource(source, limits: limits);

  final ViewIr _ir;
  final CrepusLimits _limits;
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
    final renderer = _Renderer(
      resolved,
      onAction,
      allowNetworkImages,
      _limits.maxNodes,
    );
    final children = renderer.renderList(_ir.root, data);
    return renderer.flow(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: children,
    );
  }
}

class _ScrollBranch extends StatelessWidget {
  const _ScrollBranch({
    required this.vertical,
    required this.horizontal,
    required this.child,
  });

  final bool vertical;
  final bool horizontal;
  final Widget child;

  @override
  Widget build(BuildContext context) => child;
}

class _KeyboardTap extends StatefulWidget {
  const _KeyboardTap({
    required this.onTap,
    required this.signedMargins,
    this.isLink = false,
    required this.child,
  });
  final VoidCallback onTap;
  final bool signedMargins;
  final bool isLink;
  final Widget child;

  @override
  State<_KeyboardTap> createState() => _KeyboardTapState();
}

class _KeyboardTapState extends State<_KeyboardTap> {
  final _focus = FocusNode();

  @override
  void initState() {
    super.initState();
    _focus.addListener(_focusChanged);
  }

  void _focusChanged() => setState(() {});

  @override
  void dispose() {
    _focus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final decoration = BoxDecoration(
      border: _focus.hasPrimaryFocus
          ? Border.all(color: Theme.of(context).colorScheme.primary, width: 2)
          : null,
    );
    final child = widget.signedMargins
        ? _MarginDecoratedBox(
            decoration: decoration,
            position: DecorationPosition.foreground,
            child: widget.child,
          )
        : DecoratedBox(
            decoration: decoration,
            position: DecorationPosition.foreground,
            child: widget.child,
          );
    return Shortcuts(
      shortcuts: _focus.hasPrimaryFocus
          ? const {
              SingleActivator(LogicalKeyboardKey.enter): ActivateIntent(),
              SingleActivator(LogicalKeyboardKey.numpadEnter): ActivateIntent(),
              SingleActivator(LogicalKeyboardKey.space): ActivateIntent(),
            }
          : const {},
      child: Actions(
        actions: {
          ActivateIntent: CallbackAction<ActivateIntent>(
            onInvoke: (_) {
              widget.onTap();
              return null;
            },
          ),
        },
        child: Focus(
          focusNode: _focus,
          includeSemantics: false,
          // Read focusability after Focus attaches to any ExcludeFocus ancestor.
          child: Builder(
            builder: (context) => _MarginFocusSemantics(
              link: widget.isLink,
              focusable: _focus.canRequestFocus,
              focused: _focus.canRequestFocus ? _focus.hasPrimaryFocus : null,
              onFocus:
                  defaultTargetPlatform != TargetPlatform.iOS &&
                      _focus.canRequestFocus
                  ? _focus.requestFocus
                  : null,
              child: widget.signedMargins
                  ? _MarginGesture(onTap: widget.onTap, child: child)
                  : GestureDetector(onTap: widget.onTap, child: child),
            ),
          ),
        ),
      ),
    );
  }
}

class _Renderer {
  _Renderer(
    this.theme,
    this.onAction,
    this.allowNetworkImages,
    this._remainingNodes,
  );

  int _remainingNodes;
  int _verticalScrolls = 0;
  int _horizontalScrolls = 0;

  bool hasSignedMargins = false;

  Widget flow({
    Axis axis = Axis.vertical,
    CrossAxisAlignment crossAxisAlignment = CrossAxisAlignment.start,
    MainAxisAlignment mainAxisAlignment = MainAxisAlignment.start,
    MainAxisSize mainAxisSize = MainAxisSize.min,
    required List<Widget> children,
  }) {
    // Loose flex also permits natural sizing when the host is unbounded.
    children = [
      for (final child in children)
        if (child is _ScrollBranch &&
            (axis == Axis.vertical ? child.vertical : child.horizontal))
          Flexible(child: child)
        else
          child,
    ];
    final signedMargins = hasSignedMargins;
    Widget buildFlow(CrossAxisAlignment alignment) {
      if (signedMargins) {
        return _MarginFlow(
          axis: axis,
          crossAxisAlignment: alignment,
          mainAxisAlignment: mainAxisAlignment,
          mainAxisSize: mainAxisSize,
          children: children,
        );
      }
      return axis == Axis.horizontal
          ? Row(
              mainAxisSize: mainAxisSize,
              crossAxisAlignment: alignment,
              mainAxisAlignment: mainAxisAlignment,
              children: children,
            )
          : Column(
              mainAxisSize: mainAxisSize,
              crossAxisAlignment: alignment,
              mainAxisAlignment: mainAxisAlignment,
              children: children,
            );
    }

    if (axis == Axis.vertical &&
        crossAxisAlignment == CrossAxisAlignment.stretch) {
      // A column inside a row/horizontal scroll has no finite width to stretch.
      return LayoutBuilder(
        builder: (context, constraints) => buildFlow(
          constraints.hasBoundedWidth
              ? CrossAxisAlignment.stretch
              : CrossAxisAlignment.start,
        ),
      );
    }
    return buildFlow(crossAxisAlignment);
  }

  final CrepusTheme theme;
  final CrepusActionCallback? onAction;
  final bool allowNetworkImages;

  List<Widget> renderList(List<ViewNode> nodes, Map<String, Object?> scope) {
    final widgets = <Widget>[];
    for (final node in nodes) {
      if (_remainingNodes <= 0) break;
      final rendered = render(node, scope);
      if (rendered != null) widgets.add(rendered);
    }
    return widgets;
  }

  Widget? render(ViewNode node, Map<String, Object?> scope) {
    if (_remainingNodes-- <= 0) return null;
    final verticalBefore = _verticalScrolls;
    final horizontalBefore = _horizontalScrolls;
    final widget = switch (node) {
      TextNode() => _text(node, scope),
      StackNode() => _stack(node, scope),
      ScrollNode() => _scroll(node, scope),
      ButtonNode() => _button(node, scope),
      ToggleNode() => _toggle(node, scope),
      CheckboxNode() => _checkbox(node, scope),
      ProgressNode() => _progress(node),
      TimerNode() => _timer(node),
      MeterNode() => _meter(node),
      SparklineNode() => _sparkline(node),
      BadgeNode() => _badge(node),
      DividerNode() => _divider(node),
      SpacerNode() => _spacer(node),
      ImageNode() => _image(node, scope),
      ListNode() => _list(node, scope),
      ListItemNode() => _listItem(node, scope),
      LinkNode() => _link(node, scope),
      IfNode() => _ifNode(node, scope),
      ForEachNode() => _forEach(node, scope),
      // Unknown/disallowed kinds render nothing (a faint chip in debug only).
      UnsupportedNode() => _unsupported(node),
    };
    if (widget == null) return null;
    final styled = _applyStyle(widget, node.style);
    final vertical = _verticalScrolls > verticalBefore;
    final horizontal = _horizontalScrolls > horizontalBefore;
    return vertical || horizontal
        ? _ScrollBranch(
            vertical: vertical,
            horizontal: horizontal,
            child: styled,
          )
        : styled;
  }

  // ── Leaf + container renderers ────────────────────────────────────────────

  Widget? _text(TextNode node, Map<String, Object?> scope) {
    final mixedBinding = node.bind?.startsWith('"" + ') == true;
    final bound = !mixedBinding && node.bind != null && node.bind!.isNotEmpty
        ? _lookup(node.bind!, scope)
        : null;
    final content = mixedBinding
        ? _mixedText(node.bind!, scope) ?? node.content
        : bound != null
        ? _displayValue(bound)
        : _interpolate(node.content, scope);
    if (content.isEmpty) return null;
    final style = node.style;
    final fontSize = _fontSize(style?.fontSize);
    final lineHeight = style?.lineHeight;
    final lineClamp = style?.lineClamp;
    final maxLines = style?.textOverflow == 'truncate'
        ? 1
        : (lineClamp != null && lineClamp > 0 ? lineClamp : null);
    return Text(
      content,
      style: TextStyle(
        color: _color(style?.foregroundColor) ?? theme.textColor,
        fontSize: fontSize,
        fontWeight: _weight(style?.fontWeight),
        letterSpacing: style?.letterSpacing?.isFinite == true
            ? style!.letterSpacing
            : null,
        fontStyle: style?.italic == true ? FontStyle.italic : null,
        decoration: _decoration(style),
        height: lineHeight != null && lineHeight.isFinite && lineHeight > 0
            ? math.min(lineHeight, _maxLayoutExtent / fontSize)
            : 1.35,
      ),
      textAlign: _textAlign(style?.textAlign),
      maxLines: maxLines,
      softWrap: style?.textOverflow == 'truncate' ? false : null,
      overflow: switch (style?.textOverflow) {
        'clip' => TextOverflow.clip,
        'truncate' || 'ellipsis' => TextOverflow.ellipsis,
        _ => maxLines != null ? TextOverflow.ellipsis : null,
      },
    );
  }

  Widget _stack(StackNode node, Map<String, Object?> scope) {
    final children = renderList(node.children, scope);
    final spaced = _withSpacing(children, node.spacing, node.axis);
    final widget = flow(
      axis: node.axis == StackAxis.row ? Axis.horizontal : Axis.vertical,
      crossAxisAlignment: node.axis == StackAxis.row
          ? _crossAxis(node.alignItems)
          : _crossAxisCol(node.alignItems),
      mainAxisAlignment: _mainAxis(node.justifyContent),
      children: spaced,
    );
    return _withLongPress(widget, node.onLongPress);
  }

  Widget _scroll(ScrollNode node, Map<String, Object?> scope) {
    if (node.axis == StackAxis.row) {
      _horizontalScrolls++;
    } else {
      _verticalScrolls++;
    }
    final children = renderList(node.children, scope);
    final widget = SingleChildScrollView(
      scrollDirection: node.axis == StackAxis.row
          ? Axis.horizontal
          : Axis.vertical,
      child: flow(
        axis: node.axis == StackAxis.row ? Axis.horizontal : Axis.vertical,
        children: children,
      ),
    );
    return _withLongPress(widget, node.onLongPress);
  }

  // DSL expressions decode only newline, tab, matching quote and backslash.
  String _decodeExpressionLiteral(String literal) => literal.replaceAllMapped(
    RegExp(r'\\([nt"\\])'),
    (match) => switch (match[1]) {
      'n' => '\n',
      't' => '\t',
      _ => match[1]!,
    },
  );

  bool _supportedLabelOperand(String token) =>
      _validOperand(token) &&
      (token.startsWith('"') ||
          num.tryParse(token) != null ||
          RegExp(
            r'^[a-zA-Z_$][a-zA-Z0-9_$]*(?:\.(?:[a-zA-Z_$][a-zA-Z0-9_$]*|[0-9]+))*$',
          ).hasMatch(token));

  // shortcut: unsupported operands retain snapshots until shared DSL evaluation is available.
  String? _mixedText(String binding, Map<String, Object?> scope) {
    const prefix = '"" + ';
    var offset = prefix.length;
    final out = StringBuffer();
    while (offset < binding.length) {
      while (offset < binding.length && binding[offset].trim().isEmpty) {
        offset++;
      }
      if (offset == binding.length) return null;
      final start = offset;
      if (binding[offset] == '"') {
        offset++;
        var closed = false;
        while (offset < binding.length) {
          final char = binding[offset++];
          if (char == r'\' && offset < binding.length) {
            offset++;
          } else if (char == '"') {
            closed = true;
            break;
          }
        }
        if (!closed) return null;
        final literal = binding.substring(start + 1, offset - 1);
        out.write(_decodeExpressionLiteral(literal));
      } else if (binding[offset] == '(') {
        var depth = 1;
        String? quote;
        offset++;
        final expressionStart = offset;
        while (offset < binding.length && depth > 0) {
          final char = binding[offset++];
          if (quote != null) {
            if (char == r'\' && offset < binding.length) {
              offset++;
            } else if (char == quote) {
              quote = null;
            }
          } else if (char == '"' || char == "'" || char == '`') {
            quote = char;
          } else if (char == '(') {
            depth++;
          } else if (char == ')') {
            depth--;
          }
        }
        if (depth != 0) return null;
        final token = binding.substring(expressionStart, offset - 1).trim();
        if (!_supportedLabelOperand(token)) return null;
        out.write(_displayValue(_operand(token, scope)));
      } else {
        return null;
      }
      while (offset < binding.length && binding[offset].trim().isEmpty) {
        offset++;
      }
      if (offset == binding.length) return out.toString();
      if (binding[offset++] != '+') return null;
    }
    return null;
  }

  Widget _button(ButtonNode node, Map<String, Object?> scope) {
    final supported = node.labelParts.every(
      (part) =>
          part.kind == 'literal' || _supportedLabelOperand(part.value.trim()),
    );
    // shortcut: retain unsupported snapshots until shared DSL evaluation is available.
    final label = node.labelParts.isEmpty || !supported
        ? node.label
        : node.labelParts.map((part) {
            if (part.kind == 'literal') return part.value;
            final value = _operand(part.value.trim(), scope);
            return _displayValue(value);
          }).join();
    final child = Text(label);
    final button = TextButton(
      onPressed: node.onClick?.isNotEmpty == true
          ? () => _dispatch(node.onClick)
          : null,
      style: TextButton.styleFrom(
        foregroundColor: theme.accentColor,
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      ),
      child: child,
    );
    return _withLongPress(button, node.onLongPress);
  }

  bool _checked(String? bind, bool snapshot, Map<String, Object?> scope) {
    final path = bind?.trim();
    final value = path == null || path.isEmpty ? null : _lookup(path, scope);
    return value is bool ? value : snapshot;
  }

  Widget _toggle(ToggleNode node, Map<String, Object?> scope) {
    final control = Switch(
      value: _checked(node.bind, node.checked, scope),
      onChanged: node.onChange?.isNotEmpty == true
          ? (_) => _dispatch(node.onChange)
          : null,
    );
    final row = Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        if (node.label.isNotEmpty)
          Flexible(child: Text(node.label, style: _bodyStyle())),
        control,
      ],
    );
    return MergeSemantics(child: _withLongPress(row, node.onLongPress));
  }

  Widget _checkbox(CheckboxNode node, Map<String, Object?> scope) {
    return MergeSemantics(
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Checkbox(
            value: _checked(node.bind, node.checked, scope),
            onChanged: node.onChange?.isNotEmpty == true
                ? (_) => _dispatch(node.onChange)
                : null,
          ),
          if (node.label.isNotEmpty)
            Flexible(child: Text(node.label, style: _bodyStyle())),
        ],
      ),
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
    final scale = node.min.abs() > node.max.abs()
        ? node.min.abs()
        : node.max.abs();
    final fraction = span < 0
        ? 0.0
        : span == 0
        ? (node.value >= node.max ? 1.0 : 0.0)
        : (span.isInfinite
                  ? (node.value / scale - node.min / scale) /
                        (node.max / scale - node.min / scale)
                  : (node.value - node.min) / span)
              .clamp(0, 1);
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
    return LayoutBuilder(
      builder: (context, constraints) {
        final chart = DitherSparkline(
          values: node.values,
          color: ditherColorFromName(node.color),
          variant: ditherVariantFromName(node.variant),
          height: 64,
        );
        // Full-width charts need a finite width inside rows/horizontal scrolls.
        return constraints.hasBoundedWidth
            ? chart
            : SizedBox(width: _unboundedBarWidth, child: chart);
      },
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
        final painter = TextPainter(
          text: TextSpan(
            text: label,
            style: DefaultTextStyle.of(context).style.merge(_mutedStyle()),
          ),
          textDirection: Directionality.of(context),
          textScaler: MediaQuery.textScalerOf(context),
          locale: Localizations.maybeLocaleOf(context),
        )..layout();
        final rowWidth = _unboundedBarWidth + 8 + painter.width;
        painter.dispose();
        if (constraints.maxWidth < rowWidth) {
          return Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [bar, percentText],
          );
        }
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

  // shortcut: unsupported URL expressions retain snapshots until shared DSL
  // evaluation is available, matching dynamic button labels.
  String _boundUrl(
    String snapshot,
    String? binding,
    Map<String, Object?> scope,
  ) {
    if (binding == null) return snapshot;
    final token = binding.trim();
    if (token.startsWith('"" + ')) return _mixedText(token, scope) ?? snapshot;
    if (!_supportedLabelOperand(token)) return snapshot;
    return _displayValue(_operand(token, scope));
  }

  Widget _image(ImageNode node, Map<String, Object?> scope) {
    final src = _boundUrl(node.src, node.srcBind, scope);
    final uri = Uri.tryParse(src);
    final isNetwork =
        uri != null && (uri.scheme == 'http' || uri.scheme == 'https');
    final placeholder = _imagePlaceholder(node);
    final image = isNetwork && allowNetworkImages
        ? Image.network(
            src,
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
      final markerWidget = Padding(
        padding: const EdgeInsets.only(right: 6),
        child: Text(marker, style: _mutedStyle()),
      );
      final row = hasSignedMargins
          ? _MarginMarkerRow(children: [markerWidget, rendered])
          : LayoutBuilder(
              builder: (context, constraints) {
                final bounded = constraints.hasBoundedWidth;
                return Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: bounded ? MainAxisSize.max : MainAxisSize.min,
                  children: [
                    ConstrainedBox(
                      constraints: BoxConstraints(
                        maxWidth: constraints.maxWidth,
                      ),
                      child: markerWidget,
                    ),
                    if (bounded) Flexible(child: rendered) else rendered,
                  ],
                );
              },
            );
      const padding = EdgeInsets.symmetric(vertical: 2);
      final item = hasSignedMargins
          ? _MarginPadding(padding: padding, child: row)
          : Padding(padding: padding, child: row);
      items.add(
        rendered is _ScrollBranch
            ? _ScrollBranch(
                vertical: rendered.vertical,
                horizontal: rendered.horizontal,
                child: item,
              )
            : item,
      );
      index += 1;
    }
    return flow(children: items);
  }

  Widget? _listItem(ListItemNode node, Map<String, Object?> scope) {
    final children = renderList(node.children, scope);
    if (children.isEmpty) return null;
    final widget = flow(children: children);
    return _withTap(_withLongPress(widget, node.onLongPress), node.onClick);
  }

  Widget? _link(LinkNode node, Map<String, Object?> scope) {
    final children = renderList(node.children, scope);
    if (children.isEmpty) return null;
    final widget = flow(children: children);
    return _withTap(
      widget,
      _boundUrl(node.href, node.hrefBind, scope),
      isLink: true,
    );
  }

  Widget? _ifNode(IfNode node, Map<String, Object?> scope) {
    final branch = _evalCondition(node.condition, scope)
        ? node.thenChildren
        : node.elseChildren;
    if (branch == null || branch.isEmpty) return null;
    final children = renderList(branch, scope);
    if (children.isEmpty) return null;
    return flow(children: children);
  }

  Widget? _forEach(ForEachNode node, Map<String, Object?> scope) {
    final binding = node.bind.trim();
    if (_isNullRoot(binding.split('.').first)) return null;
    final source = scope.containsKey(binding)
        ? scope[binding]
        : _lookup(binding, scope);
    if (source is! List || node.itemBody.isEmpty) return null;
    final children = <Widget>[];
    for (final item in source) {
      if (_remainingNodes <= 0) break;
      final itemScope = {...scope, node.itemName: item};
      children.addAll(renderList(node.itemBody, itemScope));
    }
    if (children.isEmpty) return null;
    return flow(children: children);
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
    return hasSignedMargins
        ? _MarginGesture(onLongPress: () => _dispatch(action), child: child)
        : GestureDetector(onLongPress: () => _dispatch(action), child: child);
  }

  Widget _withTap(Widget child, String? action, {bool isLink = false}) {
    if (action == null || action.isEmpty) return child;
    return _KeyboardTap(
      onTap: () => _dispatch(action),
      signedMargins: hasSignedMargins,
      isLink: isLink,
      child: child,
    );
  }

  // ── Style + helpers ───────────────────────────────────────────────────────

  Widget _applyStyle(Widget child, ViewStyle? style) {
    if (style == null) return child;
    var result = child;
    final padding = _padding(style);
    if (padding != null) {
      result = hasSignedMargins
          ? _MarginPadding(padding: padding, child: result)
          : Padding(padding: padding, child: result);
    }
    final bg = _color(style.backgroundColor);
    final borderColor =
        _color(style.borderColor) ??
        (style.borderWidth != null && style.borderColor == null
            ? theme.borderColor
            : null);
    final radius = style.cornerRadius;
    final safeRadius = radius != null && radius.isFinite && radius > 0
        ? _extent(radius)
        : null;
    if (bg != null || safeRadius != null || borderColor != null) {
      final borderWidth = style.borderWidth;
      final decoration = BoxDecoration(
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
      );
      result = hasSignedMargins
          ? _MarginDecoratedBox(decoration: decoration, child: result)
          : DecoratedBox(decoration: decoration, child: result);
    }
    if (style.opacity != null) {
      final opacity = style.opacity!.clamp(0, 1).toDouble();
      result = hasSignedMargins
          ? _MarginOpacity(opacity: opacity, child: result)
          : Opacity(opacity: opacity, child: result);
    }
    double? finite(double? value) => value != null && value.isFinite
        ? value.clamp(-_maxLayoutExtent, _maxLayoutExtent).toDouble()
        : null;
    final all = finite(style.margin);
    final horizontal = finite(style.marginHorizontal);
    final vertical = finite(style.marginVertical);
    final start = finite(style.marginLeft);
    final top = finite(style.marginTop);
    final end = finite(style.marginRight);
    final bottom = finite(style.marginBottom);
    final layers = <EdgeInsetsDirectional>[
      if (all != null) EdgeInsetsDirectional.all(all),
      if (horizontal != null || vertical != null)
        EdgeInsetsDirectional.fromSTEB(
          horizontal ?? 0,
          vertical ?? 0,
          horizontal ?? 0,
          vertical ?? 0,
        ),
      if ([start, top, end, bottom].any((value) => value != null))
        EdgeInsetsDirectional.fromSTEB(
          start ?? 0,
          top ?? 0,
          end ?? 0,
          bottom ?? 0,
        ),
    ];
    for (final insets in layers.reversed) {
      if (insets.isNonNegative) {
        result = hasSignedMargins
            ? _MarginPadding(padding: insets, child: result)
            : Padding(padding: insets, child: result);
      } else {
        hasSignedMargins = true;
        result = _SignedMargin(insets: insets, child: result);
      }
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
    if (weight >= 900) return FontWeight.w900;
    if (weight >= 800) return FontWeight.w800;
    if (weight >= 700) return FontWeight.w700;
    if (weight >= 600) return FontWeight.w600;
    if (weight >= 500) return FontWeight.w500;
    if (weight >= 400) return FontWeight.w400;
    if (weight >= 300) return FontWeight.w300;
    if (weight >= 200) return FontWeight.w200;
    return FontWeight.w100;
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
    'evenly' => MainAxisAlignment.spaceEvenly,
    _ => MainAxisAlignment.start,
  };

  Color? _color(String? hex) {
    if (hex == null) return null;
    final normalized = normalizeCssHex(hex);
    if (normalized == null) return null;
    var t = normalized.substring(1);
    final hasAlpha = t.length == 8;
    if (t.length == 6) t = 'ff$t';
    if (t.length == 8) {
      // View IR uses CSS RRGGBBAA; Flutter uses AARRGGBB.
      final value = int.tryParse(t, radix: 16);
      if (value == null) return null;
      // If the source had 8 digits (RRGGBBAA), rotate alpha to the front.
      if (hasAlpha) {
        final rgb = value >> 8;
        final a = value & 0xff;
        return Color((a << 24) | rgb);
      }
      return Color(value);
    }
    return null;
  }

  String _displayValue(Object? value) => switch (value) {
    double()
        when value.isFinite &&
            value.abs() < 1e15 &&
            value.truncateToDouble() == value =>
      value.toInt().toString(),
    String() || bool() || num() => '$value',
    List() => '[${value.length} items]',
    _ => '',
  };

  /// Substitute `{path}` tokens from [scope] (dotted paths and core properties). Missing
  /// keys resolve to empty string. NOT an expression evaluator.
  String _interpolate(String content, Map<String, Object?> scope) {
    if (!content.contains('{')) return content;
    return content.replaceAllMapped(RegExp(r'\{([^}]*)\}'), (match) {
      final path = match.group(1)!.trim();
      final value = _lookup(path, scope);
      return _displayValue(value);
    });
  }

  String _trimProperty(String value) {
    // Rust str::trim uses Unicode White_Space, which excludes the BOM.
    const whitespace =
        r'[\u0009-\u000D\u0020\u0085\u00A0\u1680\u2000-\u200A\u2028\u2029\u202F\u205F\u3000]';
    return value.replaceAll(RegExp('^$whitespace+|$whitespace+\$'), '');
  }

  bool _isNullRoot(String root) =>
      const {'null', 'none', 'None', 'Null'}.contains(root);

  Object? _lookup(String path, Map<String, Object?> scope) {
    final parts = path.split('.');
    if (_isNullRoot(parts.first)) return null;
    Object? current = scope[parts.first];
    for (final part in parts.skip(1)) {
      if (current is Map) {
        current = current[part];
      } else if (current is String) {
        current = switch (part) {
          'len' || 'length' => current.runes.length,
          'upper' => crepusUpperCase(current),
          'lower' => crepusLowerCase(current),
          'trim' => _trimProperty(current),
          'is_empty' || 'empty' => current.isEmpty,
          _ => null,
        };
      } else if (current is List) {
        if (part == 'len' || part == 'length') {
          current = current.length;
        } else if (part == 'is_empty' || part == 'empty') {
          current = current.isEmpty;
        } else {
          final index = RegExp(r'^[0-9]+$').hasMatch(part)
              ? int.tryParse(part)
              : null;
          current = index != null && index < current.length
              ? current[index]
              : null;
        }
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
    var quoted = false;
    var operatorIndex = -1;
    for (var idx = 0; idx < expr.length; idx++) {
      final char = expr[idx];
      if (quoted && char == r'\') {
        idx++;
        continue;
      }
      if (char == '"') {
        quoted = !quoted;
        continue;
      }
      if (quoted || idx + 1 >= expr.length) continue;
      final op = expr.substring(idx, idx + 2);
      if (op == '==' || op == '!=') {
        if (operatorIndex >= 0) return false;
        operatorIndex = idx;
        idx++;
      }
    }
    if (quoted) return false;
    if (operatorIndex >= 0) {
      final leftToken = expr.substring(0, operatorIndex).trim();
      final rightToken = expr.substring(operatorIndex + 2).trim();
      if (!_validOperand(leftToken) || !_validOperand(rightToken)) return false;
      final left = _operand(leftToken, scope);
      final right = _operand(rightToken, scope);
      if ((left is num && !left.isFinite) ||
          (right is num && !right.isFinite)) {
        return false;
      }
      final equal = _comparisonEqual(left, right);
      return expr.substring(operatorIndex, operatorIndex + 2) == '=='
          ? equal
          : !equal;
    }
    if (expr.startsWith('!')) {
      final token = expr.substring(1).trim();
      return _validOperand(token) && !_truthy(_operand(token, scope));
    }
    return _validOperand(expr) && _truthy(_operand(expr, scope));
  }

  bool _comparisonEqual(Object? left, Object? right) {
    if (left is! List && left is! Map && right is! List && right is! Map) {
      return left is num && right is num
          ? left == right
          : _comparisonText(left) == _comparisonText(right);
    }
    final pending = <Iterator<(Object?, Object?)>>[
      [(left, right)].iterator,
    ];
    // Host data can contain cycles even though JSON/Core contexts cannot.
    final seen = HashMap<Object, HashSet<Object>>.identity();
    while (pending.isNotEmpty) {
      if (!pending.last.moveNext()) {
        pending.removeLast();
        continue;
      }
      final (left, right) = pending.last.current;
      if (left is List || left is Map || right is List || right is Map) {
        if (left is List && right is List) {
          if (left.length != right.length) return false;
          if (!(seen[left] ??= HashSet<Object>.identity()).add(right)) continue;
          // Keep pair storage proportional to nesting depth, not collection width.
          pending.add(
            Iterable<(Object?, Object?)>.generate(left.length, (index) {
              final i = left.length - index - 1;
              return (left[i], right[i]);
            }).iterator,
          );
        } else if (left is Map && right is Map) {
          if (left.length != right.length) return false;
          if (!(seen[left] ??= HashSet<Object>.identity()).add(right)) continue;
          for (final key in left.keys) {
            if (!right.containsKey(key)) return false;
          }
          pending.add(
            left.entries
                .map((entry) => (entry.value, right[entry.key]))
                .iterator,
          );
        } else {
          return false;
        }
      } else if (left is num && right is num) {
        if (left != right) return false;
      } else if (_comparisonText(left) != _comparisonText(right)) {
        return false;
      }
    }
    return true;
  }

  // Core scalar comparisons coerce null to empty and spell integral doubles
  // without a trailing .0.
  // shortcut: Exponent spelling follows Dart until shared DSL scalar formatting
  // is available.
  String _comparisonText(Object? value) {
    final text = value?.toString() ?? '';
    return value is double && text.endsWith('.0')
        ? text.substring(0, text.length - 2)
        : text;
  }

  bool _validOperand(String token) {
    if (token.isEmpty || token.startsWith('!')) return false;
    if (token.startsWith('"')) {
      return RegExp(r'^"(?:\\.|[^"\\])*"$').hasMatch(token);
    }
    final number = num.tryParse(token);
    if (number != null) return number.isFinite;
    return !RegExp(r'[\s"=!<>&|()+*/{}\[\]]').hasMatch(token) &&
        token.split('.').every((part) => part.isNotEmpty);
  }

  Object? _operand(String token, Map<String, Object?> scope) {
    if (token.startsWith('"') && token.endsWith('"') && token.length >= 2) {
      return _decodeExpressionLiteral(token.substring(1, token.length - 1));
    }
    if (token == 'true') return true;
    if (token == 'false') return false;
    if (const {'null', 'none', 'None', 'Null'}.contains(token)) return null;
    final number = num.tryParse(token);
    if (number != null) return number;
    return _lookup(token, scope);
  }

  bool _truthy(Object? value) {
    if (value == null) return false;
    if (value is bool) return value;
    if (value is num) return value.isFinite && value != 0;
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
  late final Stopwatch _stopwatch = GestureBinding.instance.samplingClock
      .stopwatch();

  bool get _running => _stopwatch.isRunning;
  Duration get _elapsed {
    if (widget.node.duration <= Duration.zero) return Duration.zero;
    final elapsed = _stopwatch.elapsed;
    return elapsed > widget.node.duration ? widget.node.duration : elapsed;
  }

  /// Fine enough that the seconds figure never visibly stalls, coarse enough
  /// that an idle artifact is not repainting at frame rate.
  static const _tick = Duration(milliseconds: 200);

  bool _autostarted = false;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    if (_animationsOff) _stop();
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
    if (old.node.duration != widget.node.duration) {
      _reset();
      if (widget.node.autostart) _start();
    }
  }

  bool get _animationsOff =>
      MediaQuery.maybeOf(context)?.disableAnimations ?? false;

  void _start() {
    if (_running || widget.node.duration <= Duration.zero) return;
    if (_animationsOff) return;
    _stopwatch.start();
    _ticker?.cancel();
    _ticker = Timer.periodic(_tick, (timer) {
      if (!mounted) return;
      setState(() {
        // The stopwatch retains partial ticks and excludes time spent paused.
        if (_stopwatch.elapsed >= widget.node.duration) _stop();
      });
    });
  }

  void _stop() {
    _ticker?.cancel();
    _ticker = null;
    _stopwatch.stop();
  }

  void _reset() {
    _stop();
    if (mounted) setState(_stopwatch.reset);
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
          child: Wrap(
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              if (!done)
                TextButton(
                  onPressed: () => setState(_running ? _stop : _start),
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
