import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final (token, expected) in <(String, String)>[
    (r'''"\r"''', r'\r'),
    (r'''"\'"''', r"\'"),
    (r'''"\q"''', r'\q'),
    (r'''"\n"''', '\n'),
    (r'''"\t"''', '\t'),
    (r'''"\""''', '"'),
    (r'''"\\"''', '\\'),
  ]) {
    testWidgets('expression literal $token preserves core escapes', (
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
                  {
                    'kind': 'if',
                    'condition': 'value == $token',
                    'thenChildren': [
                      {'kind': 'text', 'content': 'Matched'},
                    ],
                    'elseChildren': [
                      {'kind': 'text', 'content': 'Mismatch'},
                    ],
                  },
                  {
                    'kind': 'button',
                    'label': 'snapshot',
                    'labelParts': [
                      {'kind': 'literal', 'value': 'Label:'},
                      {'kind': 'expr', 'value': token},
                    ],
                  },
                  {
                    'kind': 'link',
                    'href': '/snapshot',
                    'hrefBind': token,
                    'children': [
                      {'kind': 'text', 'content': 'Open'},
                    ],
                  },
                ],
              },
              data: {'value': expected},
              onAction: actions.add,
            ),
          ),
        ),
      );
      expect(find.text('Matched'), findsOneWidget);
      expect(find.text('Mismatch'), findsNothing);
      expect(find.text('Label:$expected'), findsOneWidget);
      await tester.tap(find.text('Open'));
      expect(actions, [expected]);
      expect(tester.takeException(), isNull);
    });
  }
  test(
    'source text keeps its independent carriage return and apostrophe escapes',
    () {
      final node = viewIrFromSource(r'''text "\r\'"''').root.single as TextNode;
      expect(node.content, "\r'");
    },
  );
}
