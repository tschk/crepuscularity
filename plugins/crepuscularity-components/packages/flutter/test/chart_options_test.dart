import 'dart:ui' as ui;
import 'package:crepuscularity_components/crepuscularity_components.dart';
import 'package:flutter_test/flutter_test.dart';

Future<List<int>> pixels({
  bool bars = true,
  double gap = 0.2,
  double intensity = 0,
  double dim = 1,
  bool column = false,
  double sparse = 0,
}) async {
  final recorder = ui.PictureRecorder();
  final canvas = ui.Canvas(recorder);
  if (column) {
    paintDitherColumn(
      canvas,
      0,
      0,
      20,
      seedOf(DitherColor.blue),
      DitherVariant.gradient,
      intensity: intensity,
      dim: dim,
      sparse: sparse,
    );
  } else if (bars) {
    paintBarChart(
      canvas,
      const ui.Size(80, 40),
      values: [1, 3, 2],
      color: DitherColor.blue,
      gapFraction: gap,
      intensity: intensity,
    );
  } else {
    paintSparkline(
      canvas,
      const ui.Size(80, 40),
      values: [1, 3, 2],
      color: DitherColor.blue,
      intensity: intensity,
    );
  }
  final picture = recorder.endRecording();
  final image = await picture.toImage(80, 40);
  final data = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
  final result = data!.buffer.asUint8List().toList();
  image.dispose();
  picture.dispose();
  return result;
}

void main() {
  test('nonfinite gaps retain the default chart', () async {
    final normal = await pixels();
    for (final invalid in [
      double.nan,
      double.infinity,
      double.negativeInfinity,
    ]) {
      expect(await pixels(gap: invalid), normal);
    }
  });
  test('nonfinite intensity retains the default chart', () async {
    for (final bars in [true, false]) {
      final normal = await pixels(bars: bars);
      for (final invalid in [
        double.nan,
        double.infinity,
        double.negativeInfinity,
      ]) {
        expect(await pixels(bars: bars, intensity: invalid), normal);
      }
    }
  });
  test('nonfinite dimming retains the default column', () async {
    final normal = await pixels(column: true);
    for (final invalid in [
      double.nan,
      double.infinity,
      double.negativeInfinity,
    ]) {
      expect(await pixels(column: true, dim: invalid), normal);
    }
  });
  test('finite gap and dimming extremes stay within their ranges', () async {
    expect(await pixels(gap: -1e308), await pixels(gap: 0));
    expect(await pixels(gap: 1e308), await pixels(gap: 1));
    expect(
      await pixels(column: true, dim: -1e308),
      await pixels(column: true, dim: 0),
    );
    expect(
      await pixels(column: true, dim: 1e308),
      await pixels(column: true, dim: 1),
    );
    expect(clamp01(double.nan), 0);
    expect(clamp01(double.infinity), 1);
    expect(clamp01(double.negativeInfinity), 0);
    final normal = await pixels(column: true);
    for (final invalid in [
      double.nan,
      double.infinity,
      double.negativeInfinity,
    ]) {
      expect(await pixels(column: true, sparse: invalid), normal);
    }
  });
  test('charts ignore nonfinite paint dimensions', () {
    for (final size in [
      const ui.Size(double.nan, 40),
      const ui.Size(80, double.infinity),
    ]) {
      final recorder = ui.PictureRecorder();
      final canvas = ui.Canvas(recorder);
      paintBarChart(canvas, size, values: [1, 2], color: DitherColor.blue);
      paintSparkline(canvas, size, values: [1, 2], color: DitherColor.blue);
      recorder.endRecording().dispose();
    }
  });
}
