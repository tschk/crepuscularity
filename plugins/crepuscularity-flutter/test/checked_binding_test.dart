import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final kind in ['toggle', 'checkbox']) {
    List<bool?> values(WidgetTester tester) => kind == 'toggle'
        ? tester
              .widgetList<Switch>(find.byType(Switch))
              .map((w) => w.value)
              .toList()
        : tester
              .widgetList<Checkbox>(find.byType(Checkbox))
              .map((w) => w.value)
              .toList();

    testWidgets('$kind reads nested booleans and retains snapshot fallback', (
      tester,
    ) async {
      for (final snapshot in [true, false]) {
        for (final data in <Map<String, Object?>>[
          {
            'prefs': {'enabled': true},
          },
          {
            'prefs': {'enabled': false},
          },
          {
            'prefs': {'enabled': 'false'},
          },
          {
            'prefs': {'enabled': null},
          },
          {'prefs': 0},
          {},
        ]) {
          await tester.pumpWidget(
            MaterialApp(
              home: Scaffold(
                body: CrepusView.fromIr({
                  'version': IR_VERSION,
                  'root': [
                    {
                      'kind': kind,
                      'label': 'Enabled',
                      'bind': ' prefs.enabled ',
                      'checked': snapshot,
                    },
                  ],
                }, data: data),
              ),
            ),
          );
          final prefs = data['prefs'];
          final bound = prefs is Map ? prefs['enabled'] : null;
          expect(values(tester), [bound is bool ? bound : snapshot]);
          expect(tester.takeException(), isNull);
        }
      }
    });

    testWidgets(
      '$kind uses loop scope and reflects host updates after dispatch',
      (tester) async {
        final actions = <String>[];
        Future<void> render(bool enabled) => tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: CrepusView.fromIr(
                {
                  'version': IR_VERSION,
                  'root': [
                    {
                      'kind': 'forEach',
                      'bind': 'rows',
                      'itemName': 'row',
                      'itemBody': [
                        {
                          'kind': kind,
                          'label': 'Enabled',
                          'bind': 'row.enabled',
                          'checked': false,
                          'onChange': 'prefs.enabled',
                        },
                      ],
                    },
                  ],
                },
                data: {
                  'row': {'enabled': false},
                  'rows': [
                    {'enabled': enabled},
                    {'enabled': false},
                  ],
                },
                onAction: actions.add,
              ),
            ),
          ),
        );
        await render(true);
        expect(values(tester), [true, false]);
        await tester.tap(
          find.byType(kind == 'toggle' ? Switch : Checkbox).first,
        );
        await tester.pumpAndSettle();
        expect(actions, ['prefs.enabled']);
        expect(values(tester), [true, false]);
        await render(false);
        expect(values(tester), [false, false]);
        expect(tester.takeException(), isNull);
      },
    );
  }
}
