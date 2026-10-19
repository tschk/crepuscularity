import 'dart:ui' as ui;

import 'package:crepuscularity_components/crepuscularity_components.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('scaling preserves chart pixels across numeric magnitudes', () async {
    Future<List<int>> pixels(List<double> values, {required bool bars}) async {
      final recorder = ui.PictureRecorder();
      final canvas = Canvas(recorder);
      if (bars) {
        paintBarChart(
          canvas,
          const Size(120, 60),
          values: values,
          color: DitherColor.blue,
        );
      } else {
        paintSparkline(
          canvas,
          const Size(120, 60),
          values: values,
          color: DitherColor.blue,
        );
      }
      final picture = recorder.endRecording();
      final image = await picture.toImage(120, 60);
      final bytes = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
      final result = bytes!.buffer.asUint8List().toList();
      image.dispose();
      picture.dispose();
      return result;
    }

    for (final bars in [false, true]) {
      final normal = await pixels([-1, 0, 1], bars: bars);
      final extreme = await pixels([-1e308, 0, 1e308], bars: bars);
      expect(extreme, normal);
      final tiny = await pixels([-1e-20, 0, 1e-20], bars: bars);
      expect(tiny, normal);
      expect(normal.any((channel) => channel != 0), isTrue);
    }
  });

  testWidgets('bar chart handles overflowing finite ranges', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(body: DitherBarChart(values: [-1e308, 0, 1e308])),
      ),
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('bar chart handles nonfinite samples', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: DitherBarChart(values: [1, double.nan, double.infinity, 2]),
        ),
      ),
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('sparkline tolerates nonfinite direct samples', (tester) async {
    for (final values in [
      [double.nan, 1.0, double.infinity, 2.0],
      [double.nan, double.negativeInfinity],
    ]) {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(body: DitherSparkline(values: values)),
        ),
      );
      expect(tester.takeException(), isNull);
    }
  });

  test('sparkline text excludes nonfinite numbers', () {
    expect(parseSparklineValues('1 NaN Infinity -Infinity 1e999 2'), [1, 2]);
  });

  test('resampling large finite endpoints remains finite', () {
    final values = resampleSeries([-1e308, 1e308], 3);
    expect(values, [-1e308, 0, 1e308]);
    expect(values.every((value) => value.isFinite), isTrue);
  });

  test('resampling one point repeats it across every column', () {
    expect(resampleSeries([7], 5), [7, 7, 7, 7, 7]);
    expect(resampleSeries([], 3), [0, 0, 0]);
    expect(resampleSeries([7], 0), isEmpty);
  });

  testWidgets('sparkline handles finite values with overflowing span', (
    tester,
  ) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(body: DitherSparkline(values: [-1e308, 0, 1e308])),
      ),
    );
    expect(tester.takeException(), isNull);
  });

  test('parseSparklineValues accepts comma and space separated lists', () {
    expect(parseSparklineValues('1,2,3'), [1, 2, 3]);
    expect(parseSparklineValues('1 2 3 4'), [1, 2, 3, 4]);
    expect(parseSparklineValues('[10,20,30]'), [10, 20, 30]);
  });

  test('ditherColorFromName maps aliases', () {
    expect(ditherColorFromName(' GREEN '), DitherColor.green);
    expect(ditherVariantFromName(' DOTTED '), DitherVariant.dotted);
    expect(ditherColorFromName('green'), DitherColor.green);
    expect(ditherColorFromName('gray'), DitherColor.grey);
    expect(ditherColorFromName(null), DitherColor.blue);
  });

  test('resampleSeries interpolates length', () {
    final out = resampleSeries([0, 10], 5);
    expect(out.length, 5);
    expect(out.first, 0);
    expect(out.last, 10);
  });

  testWidgets('DitherSparkline paints without error', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: DitherSparkline(
            values: [2, 4, 3, 8, 6, 9],
            color: DitherColor.green,
          ),
        ),
      ),
    );
    expect(find.byType(DitherSparkline), findsOneWidget);
  });

  testWidgets('DitherBarChart paints without error', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(body: DitherBarChart(values: [1, 3, 2, 5, 4])),
      ),
    );
    expect(find.byType(DitherBarChart), findsOneWidget);
  });
}
