import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('scroll JSON and source dispatch exact long press action once', (
    tester,
  ) async {
    for (final source in [false, true]) {
      final actions = <String>[];
      final view = source
          ? CrepusView.fromSource(
              'scroll @longpress=scroll:held\n  text "Hold"',
              onAction: actions.add,
            )
          : CrepusView.fromIr({
              'version': IR_VERSION,
              'root': [
                {
                  'kind': 'scroll',
                  'axis': 'column',
                  'onLongPress': 'scroll:held',
                  'children': [
                    {'kind': 'text', 'content': 'Hold'},
                  ],
                },
              ],
            }, onAction: actions.add);
      await tester.pumpWidget(MaterialApp(home: Scaffold(body: view)));
      await tester.longPress(find.text('Hold'));
      expect(actions, ['scroll:held'], reason: 'source=$source');
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets('scroll drag wins with and without a long press action', (
    tester,
  ) async {
    for (final action in [null, 'scroll:held']) {
      final actions = <String>[];
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SizedBox(
              height: 100,
              child: CrepusView.fromIr({
                'version': IR_VERSION,
                'root': [
                  {
                    'kind': 'scroll',
                    'axis': 'column',
                    if (action != null) 'onLongPress': action,
                    'children': [
                      {
                        'kind': 'text',
                        'content': List.filled(40, 'Line').join('\n'),
                      },
                    ],
                  },
                ],
              }, onAction: actions.add),
            ),
          ),
        ),
      );
      final state = tester.state<ScrollableState>(find.byType(Scrollable));
      expect(state.position.maxScrollExtent, greaterThan(0));
      await tester.drag(
        find.byType(SingleChildScrollView),
        const Offset(0, -80),
      );
      await tester.pumpAndSettle();
      expect(state.position.pixels, greaterThan(0));
      expect(tester.takeException(), isNull);
      expect(actions, isEmpty);
    }
  });
}
