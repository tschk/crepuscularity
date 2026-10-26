import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('source scroll viewport preserves independent content layout', () {
    for (final tag in [
      'div',
      'stack',
      'card',
      'section',
      'panel',
      'group',
      'view',
    ]) {
      for (final (classes, viewport, content) in [
        ('flex-col overflow-x-auto', StackAxis.row, StackAxis.column),
        ('flex-row overflow-y-auto', StackAxis.column, StackAxis.row),
        ('flex-row overflow-auto', StackAxis.row, StackAxis.row),
        ('flex-col overflow-auto', StackAxis.column, StackAxis.column),
        (
          'flex-row overflow-x-auto overflow-y-auto',
          StackAxis.row,
          StackAxis.row,
        ),
      ]) {
        final node =
            viewIrFromSource(
                  '$tag $classes gap-3 items-center justify-between @longpress=held\n  text "A"',
                ).root.single
                as ScrollNode;
        final axis = tag == 'view' ? StackAxis.column : content;
        final expectedViewport =
            classes.contains('overflow-x-auto') &&
                !classes.contains('overflow-y-auto')
            ? StackAxis.row
            : classes.contains('overflow-y-auto') &&
                  !classes.contains('overflow-x-auto')
            ? StackAxis.column
            : axis;
        expect(
          node.axis,
          expectedViewport,
          reason: '$tag $classes ($viewport)',
        );
        final inner = node.children.single as StackNode;
        expect(inner.axis, axis);
        expect(inner.spacing, 12);
        expect(inner.alignItems, 'center');
        expect(inner.justifyContent, 'between');
        expect(inner.style, isNull);
        expect(inner.onLongPress, isNull);
        expect(node.onLongPress, 'held');
      }
    }
    for (final source in ['scroll', 'div scroll']) {
      expect(
        viewIrFromSource('$source\n  text "A"').root.single,
        isA<ScrollNode>(),
      );
    }
    expect(
      viewIrFromSource('stack flex-row\n  text "A"').root.single,
      isA<StackNode>(),
    );
  });

  test(
    'synthetic content stack counts toward source node and depth limits',
    () {
      const source = 'div overflow-auto\n  text "A"';
      expect(
        viewIrFromSource(source, limits: const CrepusLimits(maxNodes: 2)).root,
        isEmpty,
      );
      expect(
        viewIrFromSource(source, limits: const CrepusLimits(maxDepth: 2)).root,
        isEmpty,
      );
      expect(
        viewIrFromSource(
          source,
          limits: const CrepusLimits(maxNodes: 3, maxDepth: 3),
        ).root,
        hasLength(1),
      );
      expect(
        viewIrFromSource(
          'div\n  text "A"',
          limits: const CrepusLimits(maxNodes: 2, maxDepth: 2),
        ).root,
        hasLength(1),
      );
    },
  );

  testWidgets('cross-axis source scroll drags while keeping spacing', (
    tester,
  ) async {
    for (final horizontal in [true, false]) {
      final content = horizontal ? 'W' * 200 : List.filled(40, 'A').join(r'\n');
      final source =
          'div ${horizontal ? "flex-col overflow-x-auto" : "flex-row overflow-y-auto"} gap-3\n  text "$content"\n  text "End"';
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              width: 200,
              height: 100,
              child: CrepusView.fromSource(source),
            ),
          ),
        ),
      );
      final viewport = tester.widget<SingleChildScrollView>(
        find.byType(SingleChildScrollView),
      );
      expect(
        viewport.scrollDirection,
        horizontal ? Axis.horizontal : Axis.vertical,
      );
      final inner = tester.widget<Flex>(
        find
            .ancestor(
              of: find.text('End'),
              matching: find.byWidgetPredicate((w) => w is Flex),
            )
            .first,
      );
      expect(inner.direction, horizontal ? Axis.vertical : Axis.horizontal);
      expect(
        inner.children.whereType<SizedBox>().single.width,
        horizontal ? isNull : 12,
      );
      expect(
        inner.children.whereType<SizedBox>().single.height,
        horizontal ? 12 : isNull,
      );
      final state = tester.state<ScrollableState>(find.byType(Scrollable));
      expect(state.position.maxScrollExtent, greaterThan(0));
      await tester.drag(
        find.byType(SingleChildScrollView),
        horizontal ? const Offset(-80, 0) : const Offset(0, -80),
      );
      await tester.pumpAndSettle();
      expect(state.position.pixels, greaterThan(0));
      expect(tester.takeException(), isNull);
    }
  });
}
