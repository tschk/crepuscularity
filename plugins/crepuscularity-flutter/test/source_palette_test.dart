import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('supported named source colors match shared Tailwind v4 IR', (
    tester,
  ) async {
    const palette = {
      'red-500': '#fb2c36',
      'green-500': '#00c950',
      'blue-500': '#2b7fff',
      'blue-900': '#1c398e',
      'amber-500': '#fe9a00',
      'slate-500': '#62748e',
    };
    for (final entry in palette.entries) {
      final name = entry.key;
      final source = 'text text-$name bg-$name border-$name "Palette"';
      final style = viewIrFromSource(source).root.single.style!;
      expect(style.foregroundColor, entry.value, reason: name);
      expect(style.backgroundColor, entry.value, reason: name);
      expect(style.borderColor, entry.value, reason: name);
      final ir = {
        'version': IR_VERSION,
        'root': [
          {
            'kind': 'text',
            'content': 'Palette',
            'style': {
              'foregroundColor': entry.value,
              'backgroundColor': entry.value,
              'borderColor': entry.value,
            },
          },
        ],
      };
      for (final view in [
        CrepusView.fromSource(source),
        CrepusView.fromIr(ir),
      ]) {
        await tester.pumpWidget(MaterialApp(home: Scaffold(body: view)));
        final color = Color(
          int.parse('ff${entry.value.substring(1)}', radix: 16),
        );
        expect(tester.widget<Text>(find.text('Palette')).style!.color, color);
        final decoration = tester
            .widgetList<DecoratedBox>(find.byType(DecoratedBox))
            .map((widget) => widget.decoration)
            .whereType<BoxDecoration>()
            .singleWhere((decoration) => decoration.color == color);
        expect(decoration.border, Border.all(color: color), reason: name);
      }
    }
  });
}
