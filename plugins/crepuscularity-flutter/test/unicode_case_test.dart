import 'package:crepuscularity_flutter/src/unicode_case.dart';
import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('full Unicode expansions and supplementary scripts', () {
    expect(crepusUpperCase('Straße ﬃ'), 'STRASSE FFI');
    expect(crepusLowerCase('İ'), 'i\u0307');
    expect(
      crepusUpperCase('\u{10428}\uAB70\u{1E922}'),
      '\u{10400}\u13A0\u{1E900}',
    );
    expect(
      crepusLowerCase('\u{10400}\u13A0\u{1E900}'),
      '\u{10428}\uAB70\u{1E922}',
    );
    expect(crepusUpperCase('😀\u0378'), '😀\u0378');
    expect(crepusLowerCase('😀\u0378'), '😀\u0378');
    expect(crepusUpperCase(''), '');
    expect(crepusLowerCase(''), '');
    // Strided mapping ranges must leave intervening code points unchanged.
    expect(crepusUpperCase('ĀāĂă'), 'ĀĀĂĂ');
    expect(crepusLowerCase('ĀāĂă'), 'āāăă');
  });

  test('Greek sigma context ignores combining and punctuation marks', () {
    for (final (input, expected) in <(String, String)>[
      ('Σ', 'σ'),
      ('ΟΣ', 'ος'),
      ('ΟΣΑ', 'οσα'),
      ('ΟΣ\u0301', 'ος\u0301'),
      ('ΟΣ\u0301Α', 'οσ\u0301α'),
      ("ΟΣ'", "ος'"),
      ("ΟΣ'Α", "οσ'α"),
      ('Ο-Σ', 'ο-σ'),
      ('ΟΣ-Α', 'ος-α'),
      ('ΟΣ\u0345', 'ος\u0345'),
      ('\u0345Σ', '\u0345σ'),
      ('ΟΣ\u0345Α', 'οσ\u0345α'),
    ]) {
      expect(crepusLowerCase(input), expected, reason: input);
    }
    final marks = List.filled(10000, '\u0301').join();
    expect(crepusLowerCase('ΟΣ${marks}Α'), 'οσ${marks}α');
    expect(crepusLowerCase('ΟΣ$marks'), 'ος$marks');
  });

  testWidgets('computed casing chains follow renderer host updates', (
    tester,
  ) async {
    for (final (name, expected) in [('Straße', '7'), ('ﬃ', '3')]) {
      await tester.pumpWidget(
        MaterialApp(
          home: CrepusView.fromIr(
            {
              'version': IR_VERSION,
              'root': [
                {
                  'kind': 'text',
                  'bind': 'name.upper.length',
                  'content': 'fallback',
                },
              ],
            },
            data: {'name': name},
          ),
        ),
      );
      expect(find.text(expected), findsOneWidget);
      expect(tester.takeException(), isNull);
    }
  });
}
