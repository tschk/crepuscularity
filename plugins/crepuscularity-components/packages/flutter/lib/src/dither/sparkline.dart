import 'package:flutter/material.dart';

import 'dither_paint.dart';
import 'palette.dart';

/// Full-width dithered sparkline (dither-kit port).
class DitherSparkline extends StatelessWidget {
  const DitherSparkline({
    required this.values,
    this.color = DitherColor.blue,
    this.variant = DitherVariant.gradient,
    this.height = 56,
    super.key,
  });

  final List<double> values;
  final DitherColor color;
  final DitherVariant variant;
  final double height;

  @override
  Widget build(BuildContext context) {
    if (values.length < 2) return SizedBox(height: height);
    return SizedBox(
      height: height,
      width: double.infinity,
      child: CustomPaint(
        painter: _SparklinePainter(
          values: values,
          color: color,
          variant: variant,
        ),
      ),
    );
  }
}

class _SparklinePainter extends CustomPainter {
  _SparklinePainter({
    required this.values,
    required this.color,
    required this.variant,
  });

  final List<double> values;
  final DitherColor color;
  final DitherVariant variant;

  @override
  void paint(Canvas canvas, Size size) {
    paintSparkline(
      canvas,
      size,
      values: values,
      color: color,
      variant: variant,
    );
  }

  @override
  bool shouldRepaint(covariant _SparklinePainter oldDelegate) {
    return oldDelegate.values != values ||
        oldDelegate.color != color ||
        oldDelegate.variant != variant;
  }
}

DitherVariant ditherVariantFromName(String? name) {
  return switch (name?.toLowerCase()) {
    'dotted' => DitherVariant.dotted,
    'hatched' => DitherVariant.hatched,
    'solid' => DitherVariant.solid,
    _ => DitherVariant.gradient,
  };
}

List<double> parseSparklineValues(String? raw) {
  if (raw == null || raw.trim().isEmpty) return const [];
  final cleaned = raw.replaceAll('[', '').replaceAll(']', '');
  return cleaned
      .split(RegExp(r'[,\s]+'))
      .map((part) => double.tryParse(part.trim()))
      .whereType<double>()
      .toList(growable: false);
}
