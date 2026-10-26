import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:crepuscularity_flutter/src/string_literals.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('overlapping escapes decode once', () {
    expect(unquoteCrepusLiteral(r'"\\n"'), r'\n');
    expect(unquoteCrepusLiteral(r'"\\t"'), r'\t');
    expect(unquoteCrepusLiteral(r'"\\\\"'), r'\\');
    expect(unquoteCrepusLiteral(r'"\n\r\t"'), '\n\r\t');
  });

  test('unknown escapes and Unicode remain literal', () {
    expect(unquoteCrepusLiteral(r'"雪\q"'), r'雪\q');
    expect(unquoteCrepusLiteral(r'"\"quoted\""'), '"quoted"');
    expect(unquoteCrepusLiteral('""'), '');
  });

  test('source parser preserves literal backslash-n', () {
    final ir = viewIrFromSource(r'text "\\n"');
    expect((ir.root.single as TextNode).content, r'\n');
  });
}
