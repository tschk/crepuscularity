import 'dart:ui' as ui;
import 'package:crepuscularity_components/crepuscularity_components.dart';
import 'package:flutter_test/flutter_test.dart';

Future<List<int>> pixels(List<double> values) async {
  final recorder = ui.PictureRecorder();
  paintBarChart(
    ui.Canvas(recorder),
    const ui.Size(80, 60),
    values: values,
    color: DitherColor.blue,
    variant: DitherVariant.solid,
  );
  final picture = recorder.endRecording();
  final image = await picture.toImage(80, 60);
  final bytes = await image.toByteData(format: ui.ImageByteFormat.rawRgba);
  final result = bytes!.buffer.asUint8List().toList();
  image.dispose();
  picture.dispose();
  return result;
}

int alpha(List<int> pixels, int x, int y) => pixels[(y * 80 + x) * 4 + 3];

void main() {
  test('mixed-sign bars extend in opposite directions from zero', () async {
    final data = await pixels([-1, 1]);
    expect(alpha(data, 20, 10), 0);
    expect(alpha(data, 20, 50), greaterThan(0));
    expect(alpha(data, 60, 10), greaterThan(0));
    expect(alpha(data, 60, 50), 0);
  });
  test(
    'all-negative bars include zero and grow with absolute magnitude',
    () async {
      final data = await pixels([-2, -1]);
      expect(alpha(data, 20, 10), greaterThan(0));
      expect(alpha(data, 20, 50), greaterThan(0));
      expect(alpha(data, 60, 10), greaterThan(0));
      expect(alpha(data, 60, 50), 0);
    },
  );
  test(
    'positive bars retain bottom baseline and zero retains its marker',
    () async {
      final data = await pixels([1, 2]);
      expect(alpha(data, 20, 10), 0);
      expect(alpha(data, 20, 50), greaterThan(0));
      expect(alpha(data, 60, 10), greaterThan(0));
      expect(alpha(data, 60, 50), greaterThan(0));
      final zero = await pixels([0]);
      expect(alpha(zero, 40, 10), 0);
      expect(alpha(zero, 40, 59), greaterThan(0));
    },
  );
}
