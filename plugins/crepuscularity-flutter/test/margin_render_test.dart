import 'dart:io';
import 'dart:ui' as ui;
import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';

Widget host(
  Map<String, Object?> style, {
  TextDirection direction = TextDirection.ltr,
  CrepusActionCallback? action,
  bool button = false,
  Map<String, Object?>? root,
  Key? boundary,
  double? heightLimit,
}) => MaterialApp(
  home: Scaffold(
    body: Directionality(
      textDirection: direction,
      child: Align(
        alignment: Alignment.topLeft,
        child: RepaintBoundary(
          key: boundary,
          child: SizedBox(
            width: 200,
            height: 200,
            child: Padding(
              padding: const EdgeInsets.all(40),
              child: Align(
                alignment: Alignment.topLeft,
                child: Builder(
                  builder: (context) {
                    final view = CrepusView.fromIr({
                      'version': 5,
                      'root': [
                        root ??
                            {
                              'kind': button ? 'button' : 'spacer',
                              if (button) 'label': 'tap',
                              if (button) 'onClick': 'clicked',
                              if (!button) 'size': 20,
                              'style': style,
                            },
                      ],
                    }, onAction: action);
                    return heightLimit == null
                        ? view
                        : SizedBox(height: heightLimit, child: view);
                  },
                ),
              ),
            ),
          ),
        ),
      ),
    ),
  ),
);
Finder get content => find.descendant(
  of: find.byType(CrepusView),
  matching: find.byWidgetPredicate(
    (w) => w is SizedBox && w.width == 20 && w.height == 20,
  ),
);
void main() {
  testWidgets('signed margins change size and placement in both directions', (
    tester,
  ) async {
    for (final direction in TextDirection.values) {
      for (final entry in [
        ({'margin': -4.0}, const Size(12, 12), const Offset(-4, -4)),
        (
          {'marginLeft': -4.0, 'marginRight': 2.0},
          const Size(18, 20),
          Offset(direction == TextDirection.ltr ? -4 : 2, 0),
        ),
        (
          {'margin': 2.0, 'marginHorizontal': -4.0},
          const Size(16, 24),
          const Offset(-2, 2),
        ),
        (
          {'margin': -20.0},
          Size.zero,
          Offset(direction == TextDirection.ltr ? -20 : 0, -20),
        ),
      ]) {
        await tester.pumpWidget(host(entry.$1, direction: direction));
        expect(tester.takeException(), isNull);
        expect(tester.getSize(find.byType(CrepusView)), entry.$2);
        expect(
          tester.getTopLeft(content) -
              tester.getTopLeft(find.byType(CrepusView)),
          entry.$3,
        );
      }
    }
  });
  testWidgets(
    'positive layers remain additive and invalid numbers are omitted',
    (tester) async {
      await tester.pumpWidget(
        host({'margin': 2.0, 'marginHorizontal': 3.0, 'marginTop': 4.0}),
      );
      expect(tester.getSize(find.byType(CrepusView)), const Size(30, 28));
      await tester.pumpWidget(
        host({'margin': double.nan, 'marginLeft': double.infinity}),
      );
      expect(tester.getSize(find.byType(CrepusView)), const Size(20, 20));
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets('extreme finite margins use the existing layout extent limit', (
    tester,
  ) async {
    await tester.pumpWidget(host({'marginLeft': -1e308}));
    expect(tester.getSize(find.byType(CrepusView)), const Size(0, 20));
    expect(
      tester.getTopLeft(content).dx -
          tester.getTopLeft(find.byType(CrepusView)).dx,
      -4000,
    );
    expect(tester.takeException(), isNull);
  });
  testWidgets('dry layout and intrinsic dimensions include negative margins', (
    tester,
  ) async {
    await tester.pumpWidget(host({'margin': -4.0}));
    final box = tester.renderObject<RenderBox>(find.byType(CrepusView));
    expect(
      box.getDryLayout(const BoxConstraints(maxWidth: 120, maxHeight: 120)),
      const Size(12, 12),
    );
    expect(box.getMinIntrinsicWidth(100), 12);
    expect(box.getMaxIntrinsicWidth(100), 12);
    expect(box.getMinIntrinsicHeight(100), 12);
    expect(box.getMaxIntrinsicHeight(100), 12);
  });
  testWidgets('button remains clickable outside its smaller margin slot', (
    tester,
  ) async {
    for (final style in [
      {'marginHorizontal': -8.0},
      {'margin': 2.0, 'marginHorizontal': -8.0},
      {'margin': 0.0, 'marginHorizontal': -8.0},
    ]) {
      final actions = <String>[];
      await tester.pumpWidget(host(style, button: true, action: actions.add));
      final button = tester.getRect(find.byType(TextButton));
      final root = tester.getRect(find.byType(CrepusView));
      final point = Offset(button.right - 2, button.center.dy);
      expect(point.dx, greaterThan(root.right));
      await tester.tapAt(point);
      await tester.pump();
      expect(actions, ['clicked']);
      expect(tester.takeException(), isNull);
    }
  });
  testWidgets('decorated ancestors preserve overflow button clicks', (
    tester,
  ) async {
    for (final style in [
      {'backgroundColor': '#ffffff'},
      {'opacity': 0.5},
      {'padding': 4.0},
    ]) {
      final actions = <String>[];
      await tester.pumpWidget(
        host(
          {},
          action: actions.add,
          root: {
            'kind': 'stack',
            'axis': 'column',
            'style': style,
            'children': [
              {
                'kind': 'button',
                'label': 'tap',
                'onClick': 'clicked',
                'style': {'marginHorizontal': -8.0},
              },
            ],
          },
        ),
      );
      final button = tester.getRect(find.byType(TextButton));
      final root = tester.getRect(find.byType(CrepusView));
      final point = Offset(button.right - 2, button.center.dy);
      expect(point.dx, greaterThan(root.right));
      await tester.tapAt(point);
      await tester.pump();
      expect(actions, ['clicked'], reason: '$style');
    }
  });
  testWidgets('gesture ancestors preserve overflowing child clicks', (
    tester,
  ) async {
    final semantics = tester.ensureSemantics();
    try {
      final actions = <String>[];
      await tester.pumpWidget(
        host(
          {},
          action: actions.add,
          root: {
            'kind': 'stack',
            'axis': 'column',
            'onLongPress': 'parent-held',
            'children': [
              {
                'kind': 'button',
                'label': 'tap',
                'onClick': 'clicked',
                'style': {'marginHorizontal': -8.0},
              },
            ],
          },
        ),
      );
      final button = tester.getRect(find.byType(TextButton));
      final root = tester.getRect(find.byType(CrepusView));
      final point = Offset(button.right - 2, button.center.dy);
      expect(point.dx, greaterThan(root.right));
      await tester.tapAt(point);
      await tester.pump();
      expect(actions, ['clicked']);
      var hasHoldAction = false;
      void inspect(SemanticsNode node) {
        hasHoldAction |= node.getSemanticsData().hasAction(
          ui.SemanticsAction.longPress,
        );
        node.visitChildren((child) {
          inspect(child);
          return true;
        });
      }

      void inspectOwner(PipelineOwner owner) {
        final root = owner.semanticsOwner?.rootSemanticsNode;
        if (root != null) inspect(root);
        owner.visitChildren(inspectOwner);
      }

      inspectOwner(tester.binding.rootPipelineOwner);
      expect(hasHoldAction, isTrue);
      final hold = await tester.startGesture(point);
      await tester.pump(const Duration(milliseconds: 700));
      await hold.up();
      await tester.pump();
      expect(actions, ['clicked', 'parent-held']);
      final cancelled = await tester.startGesture(point);
      await tester.pumpWidget(const SizedBox());
      await tester.pump(const Duration(seconds: 1));
      await cancelled.up();
      expect(actions, ['clicked', 'parent-held']);
      expect(tester.takeException(), isNull);
    } finally {
      semantics.dispose();
    }
  });
  testWidgets('signed flow paints when its constrained height is zero', (
    tester,
  ) async {
    const key = ValueKey('zero-height-paint');
    await tester.pumpWidget(
      host(
        {'marginHorizontal': -4.0, 'backgroundColor': '#ff0000'},
        boundary: key,
        heightLimit: 0,
      ),
    );
    expect(tester.getSize(find.byType(CrepusView)).height, 0);
    final boundary = tester.renderObject<RenderRepaintBoundary>(
      find.byKey(key),
    );
    await tester.runAsync(() async {
      final image = await boundary.toImage(pixelRatio: 1);
      final raw = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
      final data = raw!.buffer.asUint8List();
      final index = (41 * image.width + 37) * 4;
      expect(data[index], greaterThan(240));
      expect(data[index + 1], lessThan(20));
      final png = await image.toByteData(format: ui.ImageByteFormat.png);
      await File(
        '/tmp/crepus-flutter-zero-height-margin.png',
      ).writeAsBytes(png!.buffer.asUint8List());
      image.dispose();
    });
  });
  testWidgets('list marker rows preserve overflowing item clicks', (
    tester,
  ) async {
    for (final axis in ['row', 'column']) {
      final actions = <String>[];
      await tester.pumpWidget(
        host(
          {},
          action: actions.add,
          root: {
            'kind': 'stack',
            'axis': axis,
            'children': [
              {
                'kind': 'list',
                'ordered': false,
                'children': [
                  {
                    'kind': 'listItem',
                    'children': [
                      {
                        'kind': 'button',
                        'label': 'tap',
                        'onClick': 'clicked',
                        'style': {'marginHorizontal': -8.0},
                      },
                    ],
                  },
                ],
              },
            ],
          },
        ),
      );
      final button = tester.getRect(find.byType(TextButton));
      final root = tester.getRect(find.byType(CrepusView));
      final point = axis == 'row'
          ? Offset(button.right - 2, button.center.dy)
          : button.center;
      if (axis == 'row') expect(point.dx, greaterThan(root.right));
      await tester.tapAt(point);
      await tester.pump();
      expect(actions, ['clicked']);
      expect(tester.takeException(), isNull);
    }
  });
  testWidgets(
    'signed marker rows support unbounded dry layout after bounded layout',
    (tester) async {
      await tester.pumpWidget(
        host(
          {},
          root: {
            'kind': 'list',
            'ordered': false,
            'children': [
              {
                'kind': 'listItem',
                'children': [
                  {
                    'kind': 'spacer',
                    'size': 20,
                    'style': {'marginHorizontal': -4.0},
                  },
                ],
              },
            ],
          },
        ),
      );
      final box = tester.renderObject<RenderBox>(find.byType(CrepusView));
      final liveSize = box.size;
      final dry = box.getDryLayout(const BoxConstraints(maxHeight: 120));
      expect(dry.width, greaterThan(12));
      expect(dry.width, lessThan(liveSize.width));
      expect(dry.height, liveSize.height);
      expect(box.getDryLayout(box.constraints), liveSize);
      final baseline = box.getDryBaseline(
        box.constraints,
        TextBaseline.alphabetic,
      );
      expect(baseline, isNotNull);
      expect(baseline!.isFinite, isTrue);
      expect(
        box.getDryBaseline(
          const BoxConstraints(maxHeight: 120),
          TextBaseline.alphabetic,
        ),
        baseline,
      );
      expect(box.size, liveSize);
      expect(tester.takeException(), isNull);
    },
  );
  testWidgets('negative margins paint beyond the slot', (tester) async {
    const key = ValueKey('margin-paint');
    await tester.pumpWidget(
      host({'margin': -4.0, 'backgroundColor': '#ff0000'}, boundary: key),
    );
    final boundary = tester.renderObject<RenderRepaintBoundary>(
      find.byKey(key),
    );
    await tester.runAsync(() async {
      final image = await boundary.toImage(pixelRatio: 1);
      final bytes = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
      final data = bytes!.buffer.asUint8List();
      int pixel(int x, int y, int channel) =>
          data[(y * image.width + x) * 4 + channel];
      expect(pixel(37, 37, 0), greaterThan(240));
      expect(pixel(37, 37, 1), lessThan(20));
      expect(pixel(55, 55, 0), greaterThan(240));
      expect(pixel(55, 55, 1), lessThan(20));
      final png = await image.toByteData(format: ui.ImageByteFormat.png);
      await File(
        '/tmp/crepus-flutter-margin.png',
      ).writeAsBytes(png!.buffer.asUint8List());
      image.dispose();
    });
  });
}
