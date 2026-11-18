import 'dart:math' as math;
import 'dart:ui';

import 'palette.dart';

/// Ordered-dither painting primitives ported from
/// [Boring-Software-Inc/dither-kit](https://github.com/Boring-Software-Inc/dither-kit).
enum DitherVariant { gradient, dotted, hatched, solid }

const _bayer = [
  [0.0, 0.5, 0.125, 0.625],
  [0.75, 0.25, 0.875, 0.375],
  [0.1875, 0.6875, 0.0625, 0.5625],
  [0.9375, 0.4375, 0.8125, 0.3125],
];

const cellSize = 2.0;
const maxCols = 520;
const maxRows = 200;
const borderAlpha = 0.72;
const offTier = 0.4;

double clamp01(double value) => value < 0 ? 0 : value > 1 ? 1 : value;

({int cols, int rows}) backingSize(double width, double height) {
  return (
    cols: math.min(maxCols, math.max(8, (width / cellSize).round())),
    rows: math.min(maxRows, math.max(8, (height / cellSize).round())),
  );
}

List<double> resampleSeries(List<double> source, int cols) {
  if (source.isEmpty) return List<double>.filled(cols, 0);
  final out = List<double>.filled(cols, 0);
  final last = math.max(source.length - 1, 1);
  for (var c = 0; c < cols; c++) {
    final t = c / math.max(cols - 1, 1) * last;
    final i = t.floor();
    final f = t - i;
    final a = source[i];
    final b = source[math.min(i + 1, source.length - 1)];
    out[c] = a + (b - a) * f;
  }
  return out;
}

void paintDitherColumn(
  Canvas canvas,
  int x,
  int top,
  int floor,
  DitherSeed seed,
  DitherVariant variant, {
  double intensity = 0,
  double dim = 1,
  bool stacked = false,
  double sparse = 0,
}) {
  final t = top.round();
  final f = floor.round();
  final depth = f - t;
  final fill = ditherRgb(seed.fill);
  if (depth <= 0) {
    final paint = Paint()..color = fill.withValues(alpha: borderAlpha * dim);
    canvas.drawRect(Rect.fromLTWH(x.toDouble(), t.toDouble(), 1, 1), paint);
    return;
  }
  final bias = (variant == DitherVariant.dotted ? 0.12 : 0) +
      (stacked ? 0.2 : 0) -
      sparse;
  for (var y = t; y < f; y++) {
    var density = (y - t) / depth;
    if (stacked) density = 0.5 + 0.5 * density;
    if (variant == DitherVariant.hatched && ((x + y) & 3) >= 2) continue;
    final lit = variant == DitherVariant.solid ||
        density > _bayer[y & 3][x & 3] - 0.1 * intensity - bias;
    if (variant == DitherVariant.dotted && !lit) continue;
    final k = (0.3 + density * 0.7) * (1 + 0.22 * intensity);
    final alpha = clamp01((lit ? k : k * offTier) * dim);
    final paint = Paint()..color = fill.withValues(alpha: alpha);
    canvas.drawRect(Rect.fromLTWH(x.toDouble(), y.toDouble(), 1, 1), paint);
  }
  final edge = Paint()..color = fill.withValues(alpha: borderAlpha * dim);
  canvas.drawRect(Rect.fromLTWH(x.toDouble(), t.toDouble(), 1, 1), edge);
  if (depth > 1) {
    final feather = Paint()
      ..color = fill.withValues(alpha: borderAlpha * 0.5 * dim);
    canvas.drawRect(Rect.fromLTWH(x.toDouble(), (t + 1).toDouble(), 1, 1), feather);
  }
}

/// Paint a sparkline/area fill into [canvas] at logical [size].
void paintSparkline(
  Canvas canvas,
  Size size, {
  required List<double> values,
  required DitherColor color,
  DitherVariant variant = DitherVariant.gradient,
  double intensity = 0,
}) {
  if (values.isEmpty || size.width <= 0 || size.height <= 0) return;
  final backing = backingSize(size.width, size.height);
  final cols = backing.cols;
  final rows = backing.rows;
  final min = values.reduce(math.min);
  final max = values.reduce(math.max);
  final span = math.max(max - min, 1e-9);
  final resampled = resampleSeries(values, cols);
  final tops = List<int>.generate(cols, (x) {
    final normalized = (resampled[x] - min) / span;
    return (rows - 1 - normalized * (rows - 1)).round().clamp(0, rows - 1);
  });
  final floor = rows - 1;
  final seed = seedOf(color);
  canvas.save();
  canvas.scale(size.width / cols, size.height / rows);
  for (var x = 0; x < cols; x++) {
    paintDitherColumn(
      canvas,
      x,
      tops[x],
      floor,
      seed,
      variant,
      intensity: intensity,
    );
  }
  canvas.restore();
}
