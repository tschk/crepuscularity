import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final fromSource in [false, true]) {
    for (final container in ['row', 'scroll', 'bounded']) {
      testWidgets('sparkline width in $container (source: $fromSource)', (
        tester,
      ) async {
        final chart = <String, Object?>{
          'kind': 'sparkline',
          'values': [1, 4, 2, 6],
        };
        final count = container == 'scroll' ? 4 : 1;
        final root = container == 'bounded'
            ? chart
            : <String, Object?>{
                'kind': container == 'row' ? 'stack' : 'scroll',
                'axis': 'row',
                'children': List.filled(count, chart),
              };
        final prefix = switch (container) {
          'row' => 'stack row\n  ',
          'scroll' => 'scroll row\n  ',
          _ => '',
        };
        final source = List.generate(
          count,
          (_) => 'sparkline values=1,4,2,6',
        ).join('\n  ');
        final view = fromSource
            ? CrepusView.fromSource('$prefix$source')
            : CrepusView.fromIr({
                'version': IR_VERSION,
                'root': [root],
              });
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: SizedBox(width: 320, height: 160, child: view),
            ),
          ),
        );
        expect(tester.takeException(), isNull);
        final finder = find.byType(DitherSparkline);
        expect(finder, findsNWidgets(count));
        final size = tester.getSize(finder.first);
        expect(size.width, container == 'bounded' ? 320 : 120);
        expect(size.height, 64);
        if (container == 'scroll') {
          final before = tester.getTopLeft(finder.first).dx;
          await tester.drag(
            find.byType(SingleChildScrollView),
            const Offset(-80, 0),
          );
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          expect(tester.getTopLeft(finder.first).dx, lessThan(before));
          expect(
            tester
                .state<ScrollableState>(find.byType(Scrollable))
                .position
                .pixels,
            greaterThan(0),
          );
        }
      });
    }
  }
}
