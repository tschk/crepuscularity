import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Future<void> expectCondition(
  WidgetTester tester,
  String condition,
  bool expected, {
  Map<String, Object?> data = const {},
}) async {
  await tester.pumpWidget(
    MaterialApp(
      home: CrepusView.fromIr({
        'version': IR_VERSION,
        'root': [
          {
            'kind': 'if',
            'condition': condition,
            'thenChildren': [
              {'kind': 'text', 'content': 'yes'},
            ],
            'elseChildren': [
              {'kind': 'text', 'content': 'no'},
            ],
          },
        ],
      }, data: data),
    ),
  );
  expect(find.text(expected ? 'yes' : 'no'), findsOneWidget, reason: condition);
  expect(find.text(expected ? 'no' : 'yes'), findsNothing, reason: condition);
}

void main() {
  testWidgets('null literals, missing paths and data null compare as empty', (
    tester,
  ) async {
    for (final operand in [
      'null',
      'none',
      'None',
      'Null',
      'missing',
      'value',
      'None.field',
    ]) {
      for (final (comparison, expected) in [
        ('$operand == ""', true),
        ('"" == $operand', true),
        ('$operand != ""', false),
        ('$operand == "null"', false),
        ('"null" == $operand', false),
        ('$operand != "null"', true),
        ('$operand == null', true),
      ]) {
        await expectCondition(
          tester,
          comparison,
          expected,
          data: {
            'value': null,
            'None': {'field': 'spoofed'},
          },
        );
      }
    }
  });

  testWidgets(
    'nonnull scalar and numeric comparisons retain existing behavior',
    (tester) async {
      for (final (condition, expected) in [
        ('true == "true"', true),
        ('false == "false"', true),
        ('false == ""', false),
        ('null == false', false),
        ('1 == 1.0', true),
        ('1 == "1"', true),
        ('-2 == "-2"', true),
        ('"null" == ""', false),
        ('"" != null', false),
        ('object.none == "null"', true),
      ]) {
        await expectCondition(
          tester,
          condition,
          expected,
          data: {
            'object': {'none': 'null'},
          },
        );
      }
    },
  );

  testWidgets('source condition uses the same null coercion', (tester) async {
    await tester.pumpWidget(
      MaterialApp(
        home: CrepusView.fromSource(
          'if null == ""\n  text "yes"\nelse\n  text "no"',
        ),
      ),
    );
    expect(find.text('yes'), findsOneWidget);
    expect(find.text('no'), findsNothing);
  });

  testWidgets('loop scoped null comparisons refresh with host data', (
    tester,
  ) async {
    final ir = {
      'version': IR_VERSION,
      'root': [
        {
          'kind': 'forEach',
          'bind': 'rows',
          'itemName': 'row',
          'itemBody': [
            {
              'kind': 'if',
              'condition': 'row.value == ""',
              'thenChildren': [
                {'kind': 'text', 'content': 'empty'},
              ],
              'elseChildren': [
                {'kind': 'text', 'content': 'other'},
              ],
            },
          ],
        },
      ],
    };
    for (final (values, empty) in [
      (<Object?>[null, '', 'null'], 2),
      (<Object?>['null', 'x', null], 1),
    ]) {
      await tester.pumpWidget(
        MaterialApp(
          home: CrepusView.fromIr(
            ir,
            data: {
              'rows': [
                for (final value in values) {'value': value},
              ],
            },
          ),
        ),
      );
      expect(find.text('empty'), findsNWidgets(empty));
      expect(find.text('other'), findsNWidgets(3 - empty));
    }
  });
}
