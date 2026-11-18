import 'dart:ui';

/// Named palette colours for dither charts (ported from dither-kit).
enum DitherColor {
  green,
  blue,
  purple,
  pink,
  orange,
  red,
  grey,
}

typedef Rgb = (int r, int g, int b);

class DitherSeed {
  const DitherSeed({
    required this.fill,
    this.line,
    this.star,
  });

  final Rgb fill;
  final Rgb? line;
  final Rgb? star;

  Rgb get lineOrFill => line ?? fill;
  Rgb get starOrFill => star ?? fill;
}

const _palette = <DitherColor, DitherSeed>{
  DitherColor.green: DitherSeed(
    fill: (40, 210, 110),
    line: (150, 255, 180),
    star: (200, 255, 220),
  ),
  DitherColor.blue: DitherSeed(
    fill: (53, 143, 243),
    line: (150, 200, 255),
    star: (205, 228, 255),
  ),
  DitherColor.purple: DitherSeed(
    fill: (150, 110, 255),
    line: (200, 175, 255),
    star: (225, 210, 255),
  ),
  DitherColor.pink: DitherSeed(
    fill: (240, 90, 190),
    line: (255, 170, 220),
    star: (255, 205, 235),
  ),
  DitherColor.orange: DitherSeed(
    fill: (255, 150, 50),
    line: (255, 195, 130),
    star: (255, 220, 175),
  ),
  DitherColor.red: DitherSeed(
    fill: (240, 70, 70),
    line: (255, 150, 140),
    star: (255, 195, 185),
  ),
  DitherColor.grey: DitherSeed(
    fill: (92, 92, 100),
    line: (140, 140, 150),
    star: (165, 165, 175),
  ),
};

DitherColor ditherColorFromName(String? name) {
  return switch (name?.toLowerCase()) {
    'green' => DitherColor.green,
    'purple' => DitherColor.purple,
    'pink' => DitherColor.pink,
    'orange' => DitherColor.orange,
    'red' => DitherColor.red,
    'grey' || 'gray' => DitherColor.grey,
    _ => DitherColor.blue,
  };
}

DitherSeed seedOf(DitherColor color) => _palette[color]!;

Map<DitherColor, DitherSeed> get ditherPalette => Map.unmodifiable(_palette);

Color ditherRgb(Rgb rgb, {double scale = 1, double alpha = 1}) {
  final (r, g, b) = rgb;
  return Color.fromRGBO(
    (r * scale).round().clamp(0, 255),
    (g * scale).round().clamp(0, 255),
    (b * scale).round().clamp(0, 255),
    alpha.clamp(0, 1),
  );
}
