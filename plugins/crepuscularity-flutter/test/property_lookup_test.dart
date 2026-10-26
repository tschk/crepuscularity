import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('string and list properties use core names and map precedence', (
    tester,
  ) async {
    const data = <String, Object?>{
      'name': ' A😀b ',
      'blank': '',
      'nel': '\u0085x\u0085',
      'bom': ' \ufeff x \ufeff ',
      'items': [
        'first',
        {'name': 'second'},
      ],
      'empty': <Object?>[],
      'object': {'length': 'key-length', 'upper': 'key-upper', '0': 'key-zero'},
    };
    for (final (path, expected) in <(String, String)>[
      ('nel.trim', 'x'),
      ('bom.trim', '\ufeff x \ufeff'),
      ('name.len', '5'),
      ('name.length', '5'),
      ('name.upper', ' A😀B '),
      ('name.lower', ' a😀b '),
      ('name.trim', 'A😀b'),
      ('name.upper.trim.length', '3'),
      ('name.empty', 'false'),
      ('blank.is_empty', 'true'),
      ('blank.empty', 'true'),
      ('items.len', '2'),
      ('items.length', '2'),
      ('items.empty', 'false'),
      ('empty.is_empty', 'true'),
      ('empty.empty', 'true'),
      ('items.0', 'first'),
      ('items.1.name', 'second'),
      ('items.2', ''),
      ('items.-1', ''),
      ('items.9999999999999999999999999999', ''),
      ('items.missing', ''),
      ('name.missing', ''),
      ('missing.length', ''),
      ('object.length', 'key-length'),
      ('object.upper', 'key-upper'),
      ('object.0', 'key-zero'),
    ]) {
      await tester.pumpWidget(
        MaterialApp(
          home: CrepusView.fromIr({
            'version': IR_VERSION,
            'root': [
              {'kind': 'text', 'content': 'text:{$path}'},
              {
                'kind': 'button',
                'label': 'snapshot',
                'labelParts': [
                  {'kind': 'literal', 'value': 'label:'},
                  {'kind': 'expr', 'value': path},
                ],
              },
            ],
          }, data: data),
        ),
      );
      expect(find.text('text:$expected'), findsOneWidget, reason: path);
      // Negative indices are outside the supported label expression grammar.
      expect(
        find.text(path == 'items.-1' ? 'snapshot' : 'label:$expected'),
        findsOneWidget,
        reason: path,
      );
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets('computed conditions and labels follow host updates', (
    tester,
  ) async {
    for (final name in ['', '😀a']) {
      await tester.pumpWidget(
        MaterialApp(
          home: CrepusView.fromIr(
            {
              'version': IR_VERSION,
              'root': [
                {
                  'kind': 'text',
                  'bind': 'name.upper.length',
                  'content': 'snapshot',
                },
                {
                  'kind': 'button',
                  'label': 'snapshot',
                  'labelParts': [
                    {'kind': 'literal', 'value': 'length:'},
                    {'kind': 'expr', 'value': 'name.upper.length'},
                  ],
                },
                {
                  'kind': 'if',
                  'condition': 'name.is_empty',
                  'thenChildren': [
                    {'kind': 'text', 'content': 'empty'},
                  ],
                  'elseChildren': [
                    {'kind': 'text', 'content': 'filled'},
                  ],
                },
              ],
            },
            data: {'name': name},
          ),
        ),
      );
      expect(find.text(name.isEmpty ? '0' : '2'), findsOneWidget);
      expect(find.text(name.isEmpty ? 'length:0' : 'length:2'), findsOneWidget);
      expect(find.text(name.isEmpty ? 'empty' : 'filled'), findsOneWidget);
      expect(tester.takeException(), isNull);
    }
  });
}
