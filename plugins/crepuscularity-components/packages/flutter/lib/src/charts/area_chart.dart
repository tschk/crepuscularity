import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart' show listEquals;

import '../dither/dither_paint.dart';
import '../dither/palette.dart';

/// Full-width dithered area chart (same paint engine as [DitherSparkline]).
/// Rebuild after changing values; each painter snapshots the supplied list.
class DitherAreaChart extends StatelessWidget {
  const DitherAreaChart({
    required this.values,
    this.color = DitherColor.blue,
    this.variant = DitherVariant.gradient,
    this.height = 120,
    this.intensity = 0,
    super.key,
  });

  final List<double> values;
  final DitherColor color;
  final DitherVariant variant;
  final double height;
  final double intensity;

  @override
  Widget build(BuildContext context) {
    if (values.length < 2) return SizedBox(height: height);
    return SizedBox(
      height: height,
      width: double.infinity,
      child: CustomPaint(
        painter: DitherAreaPainter(
          values: values,
          color: color,
          variant: variant,
          intensity: intensity,
        ),
      ),
    );
  }
}

class DitherAreaPainter extends CustomPainter {
  DitherAreaPainter({
    required List<double> values,
    required this.color,
    required this.variant,
    this.intensity = 0,
  }) : values = List<double>.of(values);

  final List<double> values;
  final DitherColor color;
  final DitherVariant variant;
  final double intensity;

  @override
  void paint(Canvas canvas, Size size) {
    paintSparkline(
      canvas,
      size,
      values: values,
      color: color,
      variant: variant,
      intensity: intensity,
    );
  }

  @override
  bool shouldRepaint(covariant DitherAreaPainter oldDelegate) {
    return !listEquals(oldDelegate.values, values) ||
        oldDelegate.color != color ||
        oldDelegate.variant != variant ||
        oldDelegate.intensity != intensity;
  }
}

/// Paint discrete bars with the Bayer engine, ignoring nonfinite samples.
/// Bars extend from zero in either direction.
/// Gaps are bounded to 0–1; nonfinite gaps use the default 0.2.
void paintBarChart(
  Canvas canvas,
  Size size, {
  required List<double> values,
  required DitherColor color,
  DitherVariant variant = DitherVariant.gradient,
  double intensity = 0,
  double gapFraction = 0.2,
}) {
  if (values.isEmpty || !size.isFinite || size.width <= 0 || size.height <= 0)
    return;
  gapFraction = gapFraction.isFinite ? gapFraction.clamp(0.0, 1.0) : 0.2;
  var series = values.where((value) => value.isFinite).toList(growable: false);
  if (series.isEmpty) return;
  final backing = backingSize(size.width, size.height);
  final cols = backing.cols;
  final rows = backing.rows;
  var min = math.min(0.0, series.reduce(math.min));
  var max = math.max(0.0, series.reduce(math.max));
  if ((max - min).isInfinite) {
    final scale = math.max(min.abs(), max.abs());
    series = series.map((value) => value / scale).toList(growable: false);
    min /= scale;
    max /= scale;
  }
  final span = max > min ? max - min : 1.0;
  final n = series.length;
  final barWidth = math.max(1, (cols / n * (1 - gapFraction)).floor());
  final stride = cols / n;
  final seed = seedOf(color);
  final baseline = (rows - 1 - (-min / span) * (rows - 1)).round().clamp(
    0,
    rows - 1,
  );

  canvas.save();
  canvas.scale(size.width / cols, size.height / rows);
  for (var i = 0; i < n; i++) {
    final normalized = (series[i] - min) / span;
    final top = (rows - 1 - normalized * (rows - 1)).round().clamp(0, rows - 1);
    final x0 = (i * stride + (stride - barWidth) / 2).round();
    for (var x = x0; x < x0 + barWidth && x < cols; x++) {
      paintDitherColumn(
        canvas,
        x,
        math.min(top, baseline),
        math.max(top, baseline),
        seed,
        variant,
        intensity: intensity,
      );
    }
  }
  canvas.restore();
}
