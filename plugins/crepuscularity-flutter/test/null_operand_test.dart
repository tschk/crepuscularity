import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets(
    'loops reserve null roots while preserving collection lookup precedence',
    (tester) async {
      for (final alias in ['null', 'none', 'None', 'Null']) {
        for (final path in [alias, '$alias.field', '$alias.length']) {
          await tester.pumpWidget(
            MaterialApp(
              home: CrepusView.fromIr(
                {
                  'version': IR_VERSION,
                  'root': [
                    for (final binding in [
                      ' $path ',
                      'object.$alias',
                      'object.rows',
                      'missing',
                      'scalar',
                    ])
                      {
                        'kind': 'forEach',
                        'bind': binding,
                        'itemName': 'item',
                        'itemBody': [
                          {
                            'kind': 'text',
                            'bind': 'item',
                            'content': 'missing-item',
                          },
                        ],
                      },
                  ],
                },
                data: {
                  alias: ['root-leak'],
                  '$alias.field': ['field-leak'],
                  '$alias.length': ['length-leak'],
                  'object': {
                    alias: ['nested-member'],
                    'rows': ['nested-rows'],
                  },
                  'object.rows': ['direct-rows'],
                  'scalar': 'not-a-list',
                },
              ),
            ),
          );
          expect(find.text('root-leak'), findsNothing, reason: path);
          expect(find.text('field-leak'), findsNothing, reason: path);
          expect(find.text('length-leak'), findsNothing, reason: path);
          expect(find.text('nested-member'), findsOneWidget, reason: path);
          expect(find.text('direct-rows'), findsOneWidget, reason: path);
          expect(find.text('nested-rows'), findsNothing, reason: path);
          expect(find.text('missing-item'), findsNothing, reason: path);
          expect(tester.takeException(), isNull);
        }
      }
    },
  );

  testWidgets(
    'direct lookups reserve null roots but retain nested member keys',
    (tester) async {
      for (final alias in ['null', 'none', 'None', 'Null']) {
        for (final path in [alias, '$alias.field', '$alias.length']) {
          await tester.pumpWidget(
            MaterialApp(
              home: CrepusView.fromIr(
                {
                  'version': IR_VERSION,
                  'root': [
                    {'kind': 'text', 'bind': path, 'content': 'fallback'},
                    {'kind': 'text', 'content': 'interpolation:{$path}'},
                    {
                      'kind': 'text',
                      'bind': 'object.$alias',
                      'content': 'missing',
                    },
                  ],
                },
                data: {
                  alias: {'field': 'root-host', 'length': 'root-length'},
                  'object': {alias: 'nested-host'},
                },
              ),
            ),
          );
          expect(find.text('fallback'), findsOneWidget, reason: path);
          expect(find.text('interpolation:'), findsOneWidget, reason: path);
          expect(find.text('nested-host'), findsOneWidget, reason: path);
          expect(tester.takeException(), isNull);
        }
      }
    },
  );

  testWidgets('null aliases remain literals despite colliding host keys', (
    tester,
  ) async {
    for (final alias in ['null', 'none', 'None', 'Null']) {
      for (final condition in [
        alias,
        '!$alias',
        '$alias == null',
        '$alias != null',
      ]) {
        await tester.pumpWidget(
          MaterialApp(
            home: CrepusView.fromIr(
              {
                'version': IR_VERSION,
                'root': [
                  {
                    'kind': 'if',
                    'condition': condition,
                    'thenChildren': [
                      {'kind': 'text', 'content': 'then'},
                    ],
                    'elseChildren': [
                      {'kind': 'text', 'content': 'else'},
                    ],
                  },
                  {
                    'kind': 'button',
                    'label': 'snapshot',
                    'labelParts': [
                      {'kind': 'literal', 'value': 'value:'},
                      {'kind': 'expr', 'value': alias},
                    ],
                  },
                ],
              },
              data: const {
                'null': 'host-null',
                'none': 'host-none',
                'None': 'host-None',
                'Null': 'host-Null',
              },
            ),
          ),
        );
        final expected = condition.startsWith('!') || condition.contains(' == ')
            ? 'then'
            : 'else';
        expect(find.text(expected), findsOneWidget, reason: condition);
        expect(find.text('value:'), findsOneWidget, reason: alias);
        expect(tester.takeException(), isNull);
      }
    }
  });
}
