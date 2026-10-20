import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('button label parts share the node cap without increasing depth', () {
    for (final count in [2, 3]) {
      final json = <String, Object?>{
        'version': IR_VERSION,
        'root': [
          {
            'kind': 'button',
            'label': 'Snapshot',
            'labelParts': List.generate(
              count,
              (_) => {'kind': 'literal', 'value': 'x'},
            ),
          },
        ],
      };
      const limits = CrepusLimits(maxNodes: 3, maxDepth: 1);
      final typed = ViewIr.fromJson(
        json,
        limits: const CrepusLimits(maxNodes: 10),
      );
      expect(crepusExceedsLimits(typed.root, limits), count == 3);
      expect(ViewIr.fromJson(json, limits: limits).root.isEmpty, count == 3);
    }
  });

  testWidgets('button label parts use live loop scope and dispatch actions', (
    tester,
  ) async {
    final actions = <String>[];
    Future<void> render(String name) => tester.pumpWidget(
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
                      'kind': 'button',
                      'label': 'Snapshot',
                      'onClick': 'choose',
                      'labelParts': [
                        {'kind': 'literal', 'value': 'Choose {secret} '},
                        {'kind': 'expr', 'value': ' row.name '},
                        {'kind': 'literal', 'value': ' '},
                        {'kind': 'expr', 'value': 'row.count'},
                        {'kind': 'expr', 'value': 'missing.path'},
                      ],
                    },
                  ],
                },
              ],
            },
            data: {
              'secret': 'exposed',
              'row': {'name': 'outer'},
              'rows': [
                {'name': name, 'count': 2},
                {'name': 'Other', 'count': 0},
              ],
            },
            onAction: (action) => actions.add(action),
          ),
        ),
      ),
    );
    await render('Ada {secret}');
    expect(find.text('Choose {secret} Ada {secret} 2'), findsOneWidget);
    expect(find.text('Choose {secret} Other 0'), findsOneWidget);
    await tester.tap(find.text('Choose {secret} Ada {secret} 2'));
    expect(actions, ['choose']);
    await render('Grace');
    expect(find.text('Choose {secret} Grace 2'), findsOneWidget);
    expect(find.text('Choose {secret} Ada {secret} 2'), findsNothing);
    expect(tester.takeException(), isNull);
  });

  testWidgets('unsupported expressions keep snapshots and operands stay live', (
    tester,
  ) async {
    for (final entry in <(String, String, String)>[
      ('1 + 2', '3', '3'),
      ('1-2', '-1', '-1'),
      ('"constant {secret}"', 'old', 'constant {secret}'),
      ('42', 'old', '42'),
      ('true', 'old', 'true'),
      ('missing.path', 'old', ''),
    ]) {
      await tester.pumpWidget(
        MaterialApp(
          home: CrepusView.fromIr(
            {
              'version': IR_VERSION,
              'root': [
                {
                  'kind': 'button',
                  'label': entry.$2,
                  'labelParts': [
                    {'kind': 'expr', 'value': entry.$1},
                  ],
                },
              ],
            },
            data: const {'secret': 'exposed'},
          ),
        ),
      );
      expect(
        tester
            .widget<Text>(
              find.descendant(
                of: find.byType(TextButton),
                matching: find.byType(Text),
              ),
            )
            .data,
        entry.$3,
      );
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets(
    'old, empty, and malformed parts retain literal label snapshots',
    (tester) async {
      for (final fields in <Map<String, Object?>>[
        {},
        {'labelParts': []},
        {'labelParts': 'invalid'},
        {
          'labelParts': [
            {'kind': 'unknown', 'value': 'name'},
          ],
        },
        {
          'labelParts': [
            {'kind': 'expr', 'value': 2},
          ],
        },
      ]) {
        await tester.pumpWidget(
          MaterialApp(
            home: CrepusView.fromIr(
              {
                'version': IR_VERSION,
                'root': [
                  {'kind': 'button', 'label': 'Snapshot {name}', ...fields},
                ],
              },
              data: const {'name': 'Ada'},
            ),
          ),
        );
        expect(find.text('Snapshot {name}'), findsOneWidget);
        expect(tester.takeException(), isNull);
      }
    },
  );
}
