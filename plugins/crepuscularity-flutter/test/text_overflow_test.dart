import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('line limits and overflow survive source and IR round trips', () {
    for (final (classes, lines, overflow) in [
      ('line-clamp-2', 2, null),
      ('truncate line-clamp-3', 3, 'truncate'),
      ('line-clamp-2 text-clip', 2, 'clip'),
      ('text-ellipsis', null, 'ellipsis'),
      (
        'line-clamp-2 line-clamp-0 line-clamp--1 line-clamp-nope line-clamp-2147483648 line-clamp-0x3 line-clamp-2.5',
        2,
        null,
      ),
    ]) {
      final style =
          (viewIrFromSource('text $classes "Long text"').root.single
                  as TextNode)
              .style!;
      final json = style.toJson();
      expect(json['lineClamp'], lines);
      expect(json['textOverflow'], overflow);
      expect(ViewStyle.fromJson(json)!.toJson(), json);
    }
    for (final invalid in [
      0,
      -1,
      1.5,
      double.nan,
      double.infinity,
      2147483648,
      true,
      'bad',
    ]) {
      expect(ViewStyle.fromJson({'lineClamp': invalid}), isNull);
    }
  });

  for (final (classes, lines, overflow) in [
    ('line-clamp-2', 2, TextOverflow.ellipsis),
    ('truncate line-clamp-3', 1, TextOverflow.ellipsis),
    ('line-clamp-2 text-clip', 2, TextOverflow.clip),
    ('text-ellipsis', null, TextOverflow.ellipsis),
    ('text-clip', null, TextOverflow.clip),
  ]) {
    testWidgets('$classes constrains visible text from source and IR', (
      tester,
    ) async {
      const content =
          'A long paragraph repeated across many lines to verify clipping and ellipsis behavior. More text to overflow the viewport.';
      final ir = viewIrFromSource('text $classes "$content"');
      for (final view in [
        CrepusView.fromSource('text $classes "$content"'),
        CrepusView.fromIr({
          'version': IR_VERSION,
          'root': [
            {
              'kind': 'text',
              'content': content,
              'style': (ir.root.single as TextNode).style!.toJson(),
            },
          ],
        }),
      ]) {
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(body: SizedBox(width: 100, child: view)),
          ),
        );
        final text = tester.widget<Text>(find.text(content));
        expect(text.maxLines, lines);
        expect(text.overflow, overflow);
        expect(text.softWrap, classes.contains('truncate') ? false : isNull);
        final paragraph = tester.renderObject<RenderParagraph>(
          find.byType(RichText).first,
        );
        if (overflow == TextOverflow.ellipsis || lines != null) {
          expect(paragraph.didExceedMaxLines, isTrue);
        }
        expect(tester.takeException(), isNull);
      }
    });
  }
}
