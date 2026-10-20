import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('line height source hints retain multiplier units and round trip', () {
    for (final (classes, expected) in [
      ('leading-none', 1.0),
      ('leading-tight', 1.25),
      ('leading-snug', 1.375),
      ('leading-normal', 1.5),
      ('leading-relaxed', 1.625),
      ('leading-loose', 2.0),
      ('leading-[1.7]', 1.7),
      ('leading-[1.7px]', 1.7),
      ('leading-2', 8.0),
      (
        'leading-tight leading-[NaN] leading-[Infinity] leading-[-2] leading-[0] leading-nope leading-0x3 leading-[0x3]',
        1.25,
      ),
    ]) {
      final style =
          (viewIrFromSource('text $classes "Lines"').root.single as TextNode)
              .style!;
      expect(style.toJson()['lineHeight'], expected);
      expect(
        ViewStyle.fromJson(style.toJson())!.toJson()['lineHeight'],
        expected,
      );
    }
  });

  testWidgets('IR line height changes actual multiline layout safely', (
    tester,
  ) async {
    final heights = <double>[];
    for (final (value, expected) in [
      (null, 1.35),
      (1.0, 1.0),
      (2.0, 2.0),
      (0, 1.35),
      (-2, 1.35),
      (double.nan, 1.35),
      (double.infinity, 1.35),
      (1e308, 4000 / 20),
    ]) {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SingleChildScrollView(
              child: CrepusView.fromIr({
                'version': IR_VERSION,
                'root': [
                  {
                    'kind': 'text',
                    'content': 'First\nSecond',
                    'style': {'fontSize': 20, 'lineHeight': value},
                  },
                ],
              }),
            ),
          ),
        ),
      );
      final finder = find.text('First\nSecond');
      expect(tester.widget<Text>(finder).style!.height, expected);
      heights.add(tester.getSize(finder).height);
      expect(heights.last.isFinite, isTrue);
      expect(tester.takeException(), isNull);
    }
    expect(heights[2], greaterThan(heights[1]));
  });
}
