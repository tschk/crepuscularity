import 'dart:ui' as ui;
import 'package:crepuscularity_components/crepuscularity_components.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final kind in ['bar', 'area', 'sparkline']) {
    testWidgets('$kind repaints mutated data and skips equal replacements', (
      tester,
    ) async {
      final key = GlobalKey();
      final values = [1.0, 4.0, 2.0];
      Widget host(List<double> data) => MaterialApp(
        home: Scaffold(
          body: RepaintBoundary(
            key: key,
            child: SizedBox(
              width: 120,
              height: 60,
              child: switch (kind) {
                'bar' => DitherBarChart(values: data, height: 60),
                'area' => DitherAreaChart(values: data, height: 60),
                _ => DitherSparkline(values: data, height: 60),
              },
            ),
          ),
        ),
      );
      Future<List<int>> pixels() async => (await tester.runAsync(() async {
        final image =
            await (key.currentContext!.findRenderObject()!
                    as RenderRepaintBoundary)
                .toImage();
        final bytes = await image.toByteData(
          format: ui.ImageByteFormat.rawRgba,
        );
        final result = bytes!.buffer.asUint8List().toList();
        image.dispose();
        return result;
      }))!;
      CustomPainter painter() => tester
          .widget<CustomPaint>(
            find
                .descendant(
                  of: find.byKey(key),
                  matching: find.byWidgetPredicate(
                    (widget) => widget is CustomPaint && widget.painter != null,
                  ),
                )
                .first,
          )
          .painter!;
      await tester.pumpWidget(host(values));
      final firstPainter = painter();
      final before = await pixels();
      values.setAll(0, [4.0, 1.0, 4.0]);
      await tester.pumpWidget(host(values));
      final updatedPainter = painter();
      final after = await pixels();
      expect(updatedPainter.shouldRepaint(firstPainter), isTrue);
      expect(after, isNot(equals(before)));
      await tester.pumpWidget(host(List<double>.of(values)));
      expect(painter().shouldRepaint(updatedPainter), isFalse);
      expect(await pixels(), after);
      expect(tester.takeException(), isNull);
    });
  }
}
