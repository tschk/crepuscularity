import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final kind in ['button', 'toggle', 'checkbox']) {
    for (final action in <String?>[null, '', ' ', 'save']) {
      testWidgets('$kind action ${action == null ? 'null' : '"$action"'}', (
        tester,
      ) async {
        final actions = <String>[];
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: CrepusView.fromIr({
                'version': IR_VERSION,
                'root': [
                  {
                    'kind': kind,
                    'label': 'Action',
                    if (kind == 'button')
                      'onClick': action
                    else
                      'onChange': action,
                  },
                ],
              }, onAction: actions.add),
            ),
          ),
        );
        final (finder, enabled) = switch (kind) {
          'button' => (
            find.byType(TextButton),
            tester.widget<TextButton>(find.byType(TextButton)).onPressed !=
                null,
          ),
          'toggle' => (
            find.byType(Switch),
            tester.widget<Switch>(find.byType(Switch)).onChanged != null,
          ),
          _ => (
            find.byType(Checkbox),
            tester.widget<Checkbox>(find.byType(Checkbox)).onChanged != null,
          ),
        };
        final expectedEnabled = action?.isNotEmpty ?? false;
        expect(enabled, expectedEnabled);
        await tester.tap(finder);
        await tester.pump();
        expect(actions, expectedEnabled ? [action] : isEmpty);
        expect(tester.takeException(), isNull);
      });
    }
  }
}
