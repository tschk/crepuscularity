import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final source in [false, true]) {
    for (final parent in ['bounded', 'row', 'scroll']) {
      for (final signed in [false, true]) {
        testWidgets(
          'column stretch under $parent (source: $source, signed: $signed)',
          (tester) async {
            final column = <String, Object?>{
              'kind': 'stack',
              'axis': 'column',
              'alignItems': 'stretch',
              'children': [
                if (signed)
                  {
                    'kind': 'text',
                    'content': 'Sibling',
                    'style': {'marginLeft': -4},
                  },
                {'kind': 'text', 'content': 'Stretch'},
              ],
            };
            final root = parent == 'bounded'
                ? column
                : <String, Object?>{
                    'kind': parent == 'scroll' ? 'scroll' : 'stack',
                    'axis': 'row',
                    'children': [column],
                  };
            final prefix = switch (parent) {
              'row' => 'stack row\n  ',
              'scroll' => 'scroll row\n  ',
              _ => '',
            };
            final indent = parent == 'bounded' ? '  ' : '    ';
            final view = source
                ? CrepusView.fromSource(
                    '${prefix}stack col items-stretch\n${signed ? '${indent}text -ml-1 "Sibling"\n' : ''}${indent}text "Stretch"',
                  )
                : CrepusView.fromIr({
                    'version': IR_VERSION,
                    'root': [root],
                  });
            await tester.pumpWidget(
              MaterialApp(
                home: Scaffold(
                  body: SizedBox(width: 240, height: 160, child: view),
                ),
              ),
            );
            expect(tester.takeException(), isNull);
            final size = tester.getSize(find.text('Stretch'));
            expect(size.width.isFinite, isTrue);
            expect(size.height.isFinite, isTrue);
            expect(size.width, greaterThan(0));
            if (parent == 'bounded') {
              expect(size.width, 240);
              if (!signed) {
                final columnWidget = tester.widget<Column>(
                  find
                      .ancestor(
                        of: find.text('Stretch'),
                        matching: find.byType(Column),
                      )
                      .first,
                );
                expect(
                  columnWidget.crossAxisAlignment,
                  CrossAxisAlignment.stretch,
                );
              }
            }
          },
        );
      }
    }
  }
}
