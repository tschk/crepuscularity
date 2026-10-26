import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final unknown in ['roundedness', 'rounded-garbage', 'rounded-3xl']) {
    test('unknown $unknown is ignored without erasing valid radius', () {
      for (final (classes, expected) in <(String, double?)>[
        (unknown, null),
        ('rounded-[20px] $unknown', 20),
        ('$unknown rounded-lg', 12),
      ]) {
        final node = viewIrFromSource('text $classes "Radius"').root.single;
        expect(node.style?.cornerRadius, expected);
      }
    });
  }
  test('malformed arbitrary radii retain established fallback and ramp', () {
    for (final (classes, expected) in [
      ('rounded-[20px] rounded-[NaNpx]', 8.0),
      ('rounded-[-6px]', 8.0),
      ('rounded-sm', 4.0),
      ('rounded-lg', 12.0),
    ]) {
      expect(
        viewIrFromSource(
          'text $classes "Radius"',
        ).root.single.style!.cornerRadius,
        expected,
      );
    }
  });
  testWidgets('unknown rounded class preserves rendered authored radius', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: CrepusView.fromSource(
            'text bg-blue-500 p-4 rounded-[20px] rounded-garbage "Radius"',
          ),
        ),
      ),
    );
    final decorations = tester
        .widgetList<DecoratedBox>(
          find.ancestor(
            of: find.text('Radius'),
            matching: find.byType(DecoratedBox),
          ),
        )
        .map((box) => box.decoration)
        .whereType<BoxDecoration>();
    expect(
      decorations.any(
        (decoration) => decoration.borderRadius == BorderRadius.circular(20),
      ),
      isTrue,
    );
    expect(tester.takeException(), isNull);
  });
}
