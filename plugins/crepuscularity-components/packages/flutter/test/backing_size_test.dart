import 'package:crepuscularity_components/crepuscularity_components.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('backing grids bound invalid and extreme dimensions independently', () {
    for (final (dimension, cols, rows) in [
      (double.nan, 8, 8),
      (double.infinity, 520, 200),
      (double.negativeInfinity, 8, 8),
      (1e308, 520, 200),
      (-1e308, 8, 8),
      (0.0, 8, 8),
      (17.0, 9, 9),
    ]) {
      expect(backingSize(dimension, 100), (cols: cols, rows: 50));
      expect(backingSize(100, dimension), (cols: 50, rows: rows));
    }
  });

  test('valid grid rounding and saturation stay unchanged', () {
    for (final width in [0.1, 15.0, 16.0, 17.0, 60.0, 399.0, 1041.0]) {
      for (final height in [0.1, 15.0, 16.0, 17.0, 60.0, 399.0, 1041.0]) {
        final result = backingSize(width, height);
        expect(result.cols, (width / 2).round().clamp(8, 520));
        expect(result.rows, (height / 2).round().clamp(8, 200));
      }
    }
  });
}
