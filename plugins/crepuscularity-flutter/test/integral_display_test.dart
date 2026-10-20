import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final (value, expected) in <(num, String)>[
    (42.0, '42'),
    (-0.0, '0'),
    (42.5, '42.5'),
    (42, '42'),
    (1e15, '1000000000000000.0'),
  ]) {
    testWidgets('display $value as $expected across supported consumers', (
      tester,
    ) async {
      final actions = <String>[];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: CrepusView.fromIr(
              {
                'version': IR_VERSION,
                'root': [
                  {'kind': 'text', 'bind': 'number', 'content': 'snapshot'},
                  {'kind': 'text', 'content': 'Interpolated {number}'},
                  {
                    'kind': 'button',
                    'label': 'snapshot',
                    'labelParts': [
                      {'kind': 'literal', 'value': 'Button '},
                      {'kind': 'expr', 'value': 'number'},
                    ],
                  },
                  {
                    'kind': 'link',
                    'href': '/snapshot',
                    'hrefBind': 'number',
                    'children': [
                      {'kind': 'text', 'content': 'Link'},
                    ],
                  },
                ],
              },
              data: {'number': value},
              onAction: actions.add,
            ),
          ),
        ),
      );
      expect(find.text(expected), findsOneWidget);
      expect(find.text('Interpolated $expected'), findsOneWidget);
      expect(find.text('Button $expected'), findsOneWidget);
      await tester.tap(find.text('Link'));
      expect(actions, [expected]);
      expect(tester.takeException(), isNull);
    });
  }
}
