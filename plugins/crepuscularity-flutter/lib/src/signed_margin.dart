part of 'renderer.dart';

class _SignedMargin extends SingleChildRenderObjectWidget {
  const _SignedMargin({required this.insets, required super.child});
  final EdgeInsetsDirectional insets;
  @override
  _RenderSignedMargin createRenderObject(BuildContext context) =>
      _RenderSignedMargin(insets, Directionality.of(context));
  @override
  void updateRenderObject(BuildContext context, _RenderSignedMargin object) {
    if (object.insets != insets ||
        object.direction != Directionality.of(context)) {
      object.insets = insets;
      object.direction = Directionality.of(context);
      object.markNeedsLayout();
    }
  }
}

double _marginSum(double a, double b) {
  final sum = a + b;
  return sum.isFinite
      ? sum
      : (sum.isNegative ? -double.maxFinite : double.maxFinite);
}

double _marginExtent(double value) =>
    value.clamp(0.0, double.maxFinite).toDouble();

class _RenderSignedMargin extends RenderShiftedBox with _MarginShiftedOverflow {
  _RenderSignedMargin(this.insets, this.direction) : super(null);
  EdgeInsetsDirectional insets;
  TextDirection direction;
  double get horizontal => _marginSum(insets.start, insets.end);
  double get vertical => _marginSum(insets.top, insets.bottom);
  BoxConstraints adjusted(BoxConstraints c) => BoxConstraints(
    minWidth: c.minWidth == 0 ? 0 : _marginExtent(c.minWidth - horizontal),
    maxWidth: c.maxWidth.isInfinite
        ? c.maxWidth
        : _marginExtent(c.maxWidth - horizontal),
    minHeight: c.minHeight == 0 ? 0 : _marginExtent(c.minHeight - vertical),
    maxHeight: c.maxHeight.isInfinite
        ? c.maxHeight
        : _marginExtent(c.maxHeight - vertical),
  );
  Size reported(BoxConstraints c, Size childSize) => c.constrain(
    Size(
      _marginExtent(_marginSum(childSize.width, horizontal)),
      _marginExtent(_marginSum(childSize.height, vertical)),
    ),
  );
  @override
  Size computeDryLayout(BoxConstraints c) =>
      reported(c, child?.getDryLayout(adjusted(c)) ?? Size.zero);
  @override
  void performLayout() {
    child?.layout(adjusted(constraints), parentUsesSize: true);
    size = reported(constraints, child?.size ?? Size.zero);
    if (child != null) {
      (child!.parentData! as BoxParentData).offset = Offset(
        direction == TextDirection.rtl
            ? size.width - child!.size.width - insets.start
            : insets.start,
        insets.top,
      );
    }
  }

  double intrinsic(double value, double inset) =>
      _marginExtent(_marginSum(value, inset));
  @override
  double computeMinIntrinsicWidth(double height) => intrinsic(
    child?.getMinIntrinsicWidth(math.max(0.0, height - vertical)) ?? 0,
    horizontal,
  );
  @override
  double computeMaxIntrinsicWidth(double height) => intrinsic(
    child?.getMaxIntrinsicWidth(math.max(0.0, height - vertical)) ?? 0,
    horizontal,
  );
  @override
  double computeMinIntrinsicHeight(double width) => intrinsic(
    child?.getMinIntrinsicHeight(math.max(0.0, width - horizontal)) ?? 0,
    vertical,
  );
  @override
  double computeMaxIntrinsicHeight(double width) => intrinsic(
    child?.getMaxIntrinsicHeight(math.max(0.0, width - horizontal)) ?? 0,
    vertical,
  );
  @override
  double? computeDryBaseline(BoxConstraints c, TextBaseline baseline) {
    final value = child?.getDryBaseline(adjusted(c), baseline);
    return value == null ? null : value + insets.top;
  }
}

class _MarginFlow extends MultiChildRenderObjectWidget {
  const _MarginFlow({
    required this.axis,
    required this.crossAxisAlignment,
    required this.mainAxisAlignment,
    this.mainAxisSize = MainAxisSize.min,
    required super.children,
  });
  final Axis axis;
  final CrossAxisAlignment crossAxisAlignment;
  final MainAxisAlignment mainAxisAlignment;
  final MainAxisSize mainAxisSize;
  @override
  _RenderMarginFlow createRenderObject(BuildContext context) =>
      _RenderMarginFlow(
        direction: axis,
        crossAxisAlignment: crossAxisAlignment,
        mainAxisAlignment: mainAxisAlignment,
        mainAxisSize: mainAxisSize,
        textDirection: Directionality.of(context),
      );
  @override
  void updateRenderObject(BuildContext context, _RenderMarginFlow object) {
    object
      ..direction = axis
      ..crossAxisAlignment = crossAxisAlignment
      ..mainAxisAlignment = mainAxisAlignment
      ..mainAxisSize = mainAxisSize
      ..textDirection = Directionality.of(context);
  }
}

class _RenderMarginFlow extends RenderFlex {
  _RenderMarginFlow({
    required super.direction,
    required super.crossAxisAlignment,
    required super.mainAxisAlignment,
    required super.textDirection,
    required super.mainAxisSize,
  });
  @override
  void paint(PaintingContext context, Offset offset) {
    if (size.isEmpty) {
      defaultPaint(context, offset);
    } else {
      super.paint(context, offset);
    }
  }

  @override
  Rect get paintBounds {
    var bounds = super.paintBounds;
    var current = firstChild;
    while (current != null) {
      final data = current.parentData! as FlexParentData;
      bounds = bounds.expandToInclude(current.paintBounds.shift(data.offset));
      current = data.nextSibling;
    }
    return bounds;
  }

  @override
  bool hitTest(BoxHitTestResult result, {required Offset position}) {
    if (!hitTestChildren(result, position: position)) return false;
    result.add(BoxHitTestEntry(this, position));
    return true;
  }
}

// Proxy boxes must retain both painting and hit testing outside their layout slot.
mixin _MarginProxyOverflow on RenderProxyBox {
  @override
  Rect get paintBounds => child == null
      ? super.paintBounds
      : super.paintBounds.expandToInclude(child!.paintBounds);
  @override
  bool hitTest(BoxHitTestResult result, {required Offset position}) {
    if (size.contains(position))
      return super.hitTest(result, position: position);
    if (!hitTestChildren(result, position: position)) return false;
    result.add(BoxHitTestEntry(this, position));
    return true;
  }
}

class _MarginFocusSemantics extends Semantics {
  _MarginFocusSemantics({
    super.link,
    required super.focusable,
    required super.focused,
    super.onFocus,
    required super.child,
  });

  @override
  RenderSemanticsAnnotations createRenderObject(BuildContext context) =>
      _RenderMarginFocusSemantics(
        properties: properties,
        textDirection: Directionality.maybeOf(context),
      );
}

class _RenderMarginFocusSemantics extends RenderSemanticsAnnotations
    with _MarginProxyOverflow {
  _RenderMarginFocusSemantics({required super.properties, super.textDirection});
}

class _MarginDecoratedBox extends DecoratedBox {
  const _MarginDecoratedBox({
    required super.decoration,
    super.position,
    required super.child,
  });
  @override
  RenderDecoratedBox createRenderObject(BuildContext context) =>
      _RenderMarginDecoration(
        decoration: decoration,
        position: position,
        configuration: createLocalImageConfiguration(context),
      );
}

class _RenderMarginDecoration extends RenderDecoratedBox
    with _MarginProxyOverflow {
  _RenderMarginDecoration({
    required super.decoration,
    required super.position,
    required super.configuration,
  });
}

class _MarginOpacity extends Opacity {
  const _MarginOpacity({required super.opacity, required super.child});
  @override
  RenderOpacity createRenderObject(BuildContext context) =>
      _RenderMarginOpacity(
        opacity: opacity,
        alwaysIncludeSemantics: alwaysIncludeSemantics,
      );
}

class _RenderMarginOpacity extends RenderOpacity with _MarginProxyOverflow {
  _RenderMarginOpacity({
    required super.opacity,
    required super.alwaysIncludeSemantics,
  });
}

class _MarginGesture extends StatefulWidget {
  const _MarginGesture({this.onTap, this.onLongPress, required this.child});
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  final Widget child;
  @override
  State<_MarginGesture> createState() => _MarginGestureState();
}

class _MarginGestureState extends State<_MarginGesture> {
  late final TapGestureRecognizer tap = TapGestureRecognizer(debugOwner: this)
    ..onTap = () => widget.onTap?.call();
  late final LongPressGestureRecognizer hold = LongPressGestureRecognizer(
    debugOwner: this,
  )..onLongPress = () => widget.onLongPress?.call();
  void down(PointerDownEvent event) {
    if (widget.onTap != null) tap.addPointer(event);
    if (widget.onLongPress != null) hold.addPointer(event);
  }

  @override
  Widget build(BuildContext context) => _MarginPointer(
    onDown: down,
    onTap: widget.onTap,
    onLongPress: widget.onLongPress,
    child: widget.child,
  );
  @override
  void dispose() {
    tap.dispose();
    hold.dispose();
    super.dispose();
  }
}

class _MarginPointer extends SingleChildRenderObjectWidget {
  const _MarginPointer({
    required this.onDown,
    this.onTap,
    this.onLongPress,
    required super.child,
  });
  final PointerDownEventListener onDown;
  final VoidCallback? onTap;
  final VoidCallback? onLongPress;
  @override
  _RenderMarginPointer createRenderObject(BuildContext context) =>
      _RenderMarginPointer(
        onPointerDown: onDown,
        tap: onTap,
        hold: onLongPress,
      );
  @override
  void updateRenderObject(BuildContext context, _RenderMarginPointer object) {
    object.onPointerDown = onDown;
    object.updateActions(onTap, onLongPress);
  }
}

class _RenderMarginPointer extends RenderPointerListener
    with _MarginProxyOverflow {
  _RenderMarginPointer({
    required super.onPointerDown,
    VoidCallback? tap,
    VoidCallback? hold,
  }) : tapAction = tap,
       holdAction = hold;
  VoidCallback? tapAction;
  VoidCallback? holdAction;
  void updateActions(VoidCallback? tap, VoidCallback? hold) {
    if (tapAction == tap && holdAction == hold) return;
    tapAction = tap;
    holdAction = hold;
    markNeedsSemanticsUpdate();
  }

  @override
  void describeSemanticsConfiguration(SemanticsConfiguration config) {
    super.describeSemanticsConfiguration(config);
    if (tapAction != null) config.onTap = tapAction;
    if (holdAction != null) config.onLongPress = holdAction;
  }
}

mixin _MarginShiftedOverflow on RenderShiftedBox {
  @override
  Rect get paintBounds => child == null
      ? super.paintBounds
      : super.paintBounds.expandToInclude(
          child!.paintBounds.shift(
            (child!.parentData! as BoxParentData).offset,
          ),
        );
  @override
  bool hitTest(BoxHitTestResult result, {required Offset position}) {
    if (!hitTestChildren(result, position: position)) return false;
    result.add(BoxHitTestEntry(this, position));
    return true;
  }
}

class _MarginPadding extends Padding {
  const _MarginPadding({required super.padding, required super.child});
  @override
  RenderPadding createRenderObject(BuildContext context) =>
      _RenderMarginPadding(
        padding: padding,
        textDirection: Directionality.maybeOf(context),
      );
}

class _RenderMarginPadding extends RenderPadding with _MarginShiftedOverflow {
  _RenderMarginPadding({required super.padding, required super.textDirection});
}

class _MarginMarkerRow extends MultiChildRenderObjectWidget {
  const _MarginMarkerRow({required super.children});
  @override
  _RenderMarginMarkerRow createRenderObject(BuildContext context) =>
      _RenderMarginMarkerRow(Directionality.of(context));
  @override
  void updateRenderObject(BuildContext context, _RenderMarginMarkerRow object) {
    object.textDirection = Directionality.of(context);
  }
}

class _RenderMarginMarkerRow extends _RenderMarginFlow {
  _RenderMarginMarkerRow(TextDirection textDirection)
    : super(
        direction: Axis.horizontal,
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisAlignment: MainAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        textDirection: textDirection,
      );
  (Size, Size, BoxConstraints) dryChildren(BoxConstraints c) {
    final marker = firstChild?.getDryLayout(c.loosen()) ?? Size.zero;
    final itemConstraints = BoxConstraints(
      maxWidth: c.hasBoundedWidth
          ? math.max(0.0, c.maxWidth - marker.width)
          : double.infinity,
      maxHeight: c.maxHeight,
    );
    final item = lastChild == firstChild
        ? Size.zero
        : lastChild?.getDryLayout(itemConstraints) ?? Size.zero;
    return (marker, item, itemConstraints);
  }

  double intrinsicHeight(double width, {required bool minimum}) {
    final marker = firstChild;
    final markerWidth = math.min(
      width,
      marker?.getMaxIntrinsicWidth(double.infinity) ?? 0.0,
    );
    final item = lastChild == marker ? null : lastChild;
    final itemWidth = math.max(0.0, width - markerWidth);
    final markerHeight = minimum
        ? marker?.getMinIntrinsicHeight(markerWidth)
        : marker?.getMaxIntrinsicHeight(markerWidth);
    final itemHeight = minimum
        ? item?.getMinIntrinsicHeight(itemWidth)
        : item?.getMaxIntrinsicHeight(itemWidth);
    return math.max(markerHeight ?? 0.0, itemHeight ?? 0.0);
  }

  @override
  double computeMinIntrinsicHeight(double width) =>
      intrinsicHeight(width, minimum: true);

  @override
  double computeMaxIntrinsicHeight(double width) =>
      intrinsicHeight(width, minimum: false);

  @override
  Size computeDryLayout(BoxConstraints c) {
    final (marker, item, _) = dryChildren(c);
    return c.constrain(
      Size(
        c.hasBoundedWidth ? c.maxWidth : marker.width + item.width,
        math.max(marker.height, item.height),
      ),
    );
  }

  @override
  double? computeDryBaseline(BoxConstraints c, TextBaseline baseline) {
    final (_, _, itemConstraints) = dryChildren(c);
    final marker = firstChild?.getDryBaseline(c.loosen(), baseline);
    final item = lastChild == firstChild
        ? null
        : lastChild?.getDryBaseline(itemConstraints, baseline);
    if (marker == null) return item;
    if (item == null) return marker;
    return math.min(marker, item);
  }

  @override
  void performLayout() {
    final marker = firstChild;
    marker?.layout(constraints.loosen(), parentUsesSize: true);
    final markerSize = marker?.size ?? Size.zero;
    final item = lastChild == marker ? null : lastChild;
    item?.layout(
      BoxConstraints(
        maxWidth: constraints.hasBoundedWidth
            ? math.max(0.0, constraints.maxWidth - markerSize.width)
            : double.infinity,
        maxHeight: constraints.maxHeight,
      ),
      parentUsesSize: true,
    );
    final itemSize = item?.size ?? Size.zero;
    size = constraints.constrain(
      Size(
        constraints.hasBoundedWidth
            ? constraints.maxWidth
            : markerSize.width + itemSize.width,
        math.max(markerSize.height, itemSize.height),
      ),
    );
    final rtl = textDirection == TextDirection.rtl;
    if (marker != null) {
      (marker.parentData! as FlexParentData).offset = Offset(
        rtl ? size.width - markerSize.width : 0.0,
        0.0,
      );
    }
    if (item != null) {
      (item.parentData! as FlexParentData).offset = Offset(
        rtl ? size.width - markerSize.width - itemSize.width : markerSize.width,
        0.0,
      );
    }
  }
}
