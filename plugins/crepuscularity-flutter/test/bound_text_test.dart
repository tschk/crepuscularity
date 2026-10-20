import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets(
    'resolved text bindings preserve literal braces in their values',
    (tester) async {
      await tester.pumpWidget(
        MaterialApp(
          home: CrepusView.fromIr(
            {
              'version': IR_VERSION,
              'root': [
                {
                  'kind': 'text',
                  'bind': 'value',
                  'content': 'Fallback {secret}',
                },
              ],
            },
            data: const {
              'value': 'Literal {secret}',
              'secret': 'reinterpreted',
            },
          ),
        ),
      );
      expect(find.text('Literal {secret}'), findsOneWidget);
      expect(find.text('Literal reinterpreted'), findsNothing);
      expect(tester.takeException(), isNull);
    },
  );

  testWidgets('missing bindings still interpolate the fallback template', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: CrepusView.fromIr(
          {
            'version': IR_VERSION,
            'root': [
              {'kind': 'text', 'bind': 'missing', 'content': 'Fallback {name}'},
            ],
          },
          data: const {'name': 'Ada'},
        ),
      ),
    );
    expect(find.text('Fallback Ada'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });
}
