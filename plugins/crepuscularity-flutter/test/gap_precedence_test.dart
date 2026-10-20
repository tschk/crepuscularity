import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final tag in ['stack', 'scroll']) {
    for (final (classes, expected) in [
      ('gap-2 gap-[2.5px]', 2.5),
      ('gap-[2.5px] gap-2', 8.0),
      ('gap-2 gap-[2.5px] gap-NaN gap-[Infinity]', 2.5),
      ('gap-2 gap-0', 0.0),
    ]) {
      testWidgets('$tag $classes uses last valid gap', (tester) async {
        final source = '$tag $classes\n  text "A"\n  text "B"';
        final root = viewIrFromSource(source).root.single;
        final stack = root is ScrollNode
            ? root.children.single as StackNode
            : root as StackNode;
        expect(stack.spacing, expected);
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: SizedBox(
                width: 240,
                height: 160,
                child: CrepusView.fromSource(source),
              ),
            ),
          ),
        );
        final bottom = tester.getBottomLeft(find.text('A')).dy;
        final top = tester.getTopLeft(find.text('B')).dy;
        expect(top - bottom, closeTo(expected, 0.001));
        expect(tester.takeException(), isNull);
      });
    }
  }
}
