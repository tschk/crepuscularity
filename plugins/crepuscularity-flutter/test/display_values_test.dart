import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  final cases = <(Object?, String)>[
    (true, 'true'),
    (false, 'false'),
    (42, '42'),
    (2.5, '2.5'),
    ('Literal {secret}', 'Literal {secret}'),
    (<Object?>[], '[0 items]'),
    ([1, 2], '[2 items]'),
    (<String, Object?>{}, ''),
    ({'private': 'hidden'}, ''),
    (null, ''),
  ];

  testWidgets('bound text, interpolation and button parts use core display', (
    tester,
  ) async {
    for (final (value, expected) in cases) {
      await tester.pumpWidget(
        MaterialApp(
          home: CrepusView.fromIr(
            {
              'version': IR_VERSION,
              'root': [
                {'kind': 'text', 'bind': 'value', 'content': 'fallback'},
                {'kind': 'text', 'content': 'text:{value}'},
                {
                  'kind': 'button',
                  'label': 'snapshot',
                  'labelParts': [
                    {'kind': 'literal', 'value': 'label:{secret}:'},
                    {'kind': 'expr', 'value': 'value'},
                  ],
                },
              ],
            },
            data: {'value': value, 'secret': 'exposed'},
          ),
        ),
      );
      if (value == null) {
        expect(find.text('fallback'), findsOneWidget);
      } else if (expected.isNotEmpty) {
        expect(find.text(expected), findsOneWidget);
      } else {
        expect(find.text('fallback'), findsNothing);
      }
      expect(find.text('text:$expected'), findsOneWidget);
      expect(find.text('label:{secret}:$expected'), findsOneWidget);
      expect(find.textContaining('hidden'), findsNothing);
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets('source loop text and button labels reflect host display updates', (
    tester,
  ) async {
    for (final (value, expected) in cases) {
      await tester.pumpWidget(
        MaterialApp(
          home: CrepusView.fromSource(
            'for row in rows\n  text "text:{row.value}"\n  button "label:{row.value}"',
            data: {
              'rows': [
                {'value': value},
              ],
              'secret': 'exposed',
            },
          ),
        ),
      );
      expect(find.text('text:$expected'), findsOneWidget);
      expect(find.text('label:$expected'), findsOneWidget);
      expect(tester.takeException(), isNull);
    }
  });
}
