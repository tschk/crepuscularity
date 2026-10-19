import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets(
    'canonical quoted chunks preserve DSL expression unknown escapes',
    (tester) async {
      for (final count in [1, 2]) {
        await tester.pumpWidget(
          MaterialApp(
            home: CrepusView.fromIr(
              {
                'version': IR_VERSION,
                'root': [
                  {
                    'kind': 'text',
                    'content': 'snapshot',
                    'bind': r'''"" + "\r\'\q" + (count)''',
                  },
                ],
              },
              data: {'count': count},
            ),
          ),
        );
        expect(find.text(r"\r\'\q" + '$count'), findsOneWidget);
        expect(tester.takeException(), isNull);
      }
    },
  );

  testWidgets('canonical text concatenation follows host and loop scope', (
    tester,
  ) async {
    for (final name in ['Ada', 'Grace {secret}']) {
      await tester.pumpWidget(
        MaterialApp(
          home: CrepusView.fromIr(
            {
              'version': IR_VERSION,
              'root': [
                {
                  'kind': 'text',
                  'content': 'snapshot',
                  'bind': r'''"" + ("quoted ) value ") + (name)''',
                },
                {
                  'kind': 'text',
                  'content': 'snapshot',
                  'bind': r'''"" + "Hello " + (name) + ": " + (count)''',
                },
                {
                  'kind': 'forEach',
                  'bind': 'rows',
                  'itemName': 'row',
                  'itemBody': [
                    {
                      'kind': 'text',
                      'content': 'snapshot',
                      'bind':
                          r'''"" + "Row " + (row.name) + ": " + (row.count)''',
                    },
                  ],
                },
              ],
            },
            data: {
              'name': name,
              'count': 2,
              'secret': 'exposed',
              'rows': [
                {'name': name, 'count': 3},
              ],
            },
          ),
        ),
      );
      expect(find.text('quoted ) value $name'), findsOneWidget);
      expect(find.text('Hello $name: 2'), findsOneWidget);
      expect(find.text('Row $name: 3'), findsOneWidget);
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets(
    'canonical operands use display semantics and keep literal escapes',
    (tester) async {
      for (final (value, expected) in <(Object?, String)>[
        (true, 'true'),
        (false, 'false'),
        ([1, 2], '[2 items]'),
        (<Object?>[], '[0 items]'),
        (null, ''),
        ({'secret': 'hidden'}, ''),
      ]) {
        await tester.pumpWidget(
          MaterialApp(
            home: CrepusView.fromIr(
              {
                'version': IR_VERSION,
                'root': [
                  {
                    'kind': 'text',
                    'content': 'snapshot',
                    'bind': r'''"" + "\"literal\"\\{secret}
	" + (value) + "|" + (missing.path)''',
                  },
                ],
              },
              data: {'value': value, 'secret': 'exposed'},
            ),
          ),
        );
        expect(find.text('"literal"\\{secret}\n\t$expected|'), findsOneWidget);
        expect(tester.takeException(), isNull);
      }
    },
  );

  testWidgets('unsupported or malformed canonical forms keep whole snapshot', (
    tester,
  ) async {
    for (final bind in [
      r'''"" + "Prefix " + (1 + 2)''',
      r'''"" + "Prefix " + (name) trailing''',
      r'''"" + "Prefix " + (name''',
      r'''"" + "unterminated''',
      r'''"" + "Prefix " + ''',
      r'''"" + ("quoted ) value") + (name) + (unknown())''',
    ]) {
      await tester.pumpWidget(
        MaterialApp(
          home: CrepusView.fromIr(
            {
              'version': IR_VERSION,
              'root': [
                {'kind': 'text', 'bind': bind, 'content': 'Snapshot {secret}'},
              ],
            },
            data: const {'name': 'Ada', 'secret': 'exposed'},
          ),
        ),
      );
      expect(find.text('Snapshot {secret}'), findsOneWidget, reason: bind);
      expect(tester.takeException(), isNull);
    }
  });
}
