import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('viewIrFromSource', () {
    test('alignment and justification preserve recognized class order', () {
      for (final (classes, align, justify) in [
        ('items-center items-end justify-center justify-start', 'end', 'start'),
        (
          'items-end items-center justify-start justify-center',
          'center',
          'center',
        ),
        ('items-stretch ignored justify-between unknown', 'stretch', 'between'),
      ]) {
        final node =
            viewIrFromSource('stack row $classes').root.single as StackNode;
        expect(node.alignItems, align);
        expect(node.justifyContent, justify);
      }
    });

    test('source opacity uses finite clamped percentages in class order', () {
      for (final (token, expected) in [
        ('opacity-25', 0.25),
        ('opacity-12.5', 0.125),
        ('opacity--20', 0.0),
        ('opacity-120', 1.0),
      ]) {
        final node =
            viewIrFromSource('text $token "Opacity"').root.single as TextNode;
        expect(node.style!.opacity, expected);
      }
      final node =
          viewIrFromSource(
                'text opacity-25 opacity-50 opacity-NaN opacity-Infinity "Opacity"',
              ).root.single
              as TextNode;
      expect(node.style!.opacity, 0.5);
    });

    test('border widths and colors stay distinct and preserve class order', () {
      final node =
          viewIrFromSource(
                'text border border-2 border-[3.5px] border-red-500 "Border"',
              ).root.single
              as TextNode;
      expect(node.style!.borderWidth, 3.5);
      expect(node.style!.borderColor, '#fb2c36');
      final invalid =
          viewIrFromSource('text border-NaN border-[-2px] "Border"').root.single
              as TextNode;
      expect(invalid.style?.borderWidth, isNull);
    });

    test('arbitrary font sizes stay distinct from text colors', () {
      final node =
          viewIrFromSource('text text-[18.5px] text-[#f00] "Font"').root.single
              as TextNode;
      expect(node.style!.fontSize, 18.5);
      expect(node.style!.foregroundColor, '#ff0000');
      for (final value in ['NaNpx', '-2px', 'Infinitypx']) {
        final invalid =
            viewIrFromSource('text text-[$value] "Font"').root.single
                as TextNode;
        expect(invalid.style?.fontSize, isNull);
      }
    });

    test('arbitrary pixel radii retain the legacy invalid-class fallback', () {
      for (final (token, expected) in [
        ('rounded-[6px]', 6.0),
        ('rounded-[2.5]', 2.5),
        ('rounded-[NaNpx]', 8.0),
        ('rounded-[-6px]', 8.0),
      ]) {
        final node =
            viewIrFromSource('text $token "Radius"').root.single as TextNode;
        expect(node.style!.cornerRadius, expected);
      }
    });

    test(
      'arbitrary CSS hex colors preserve alpha and reject malformed tokens',
      () {
        final text =
            viewIrFromSource(
                  'text text-[#ff000080] bg-[#123456] "Hex"',
                ).root.single
                as TextNode;
        expect(text.style!.foregroundColor, '#ff000080');
        expect(text.style!.backgroundColor, '#123456');
        for (final color in [
          '[#nope00]',
          '[#12345]',
          '[#1234567]',
          '[#123456789]',
        ]) {
          final node =
              viewIrFromSource('text text-$color "Hex"').root.single
                  as TextNode;
          expect(node.style?.foregroundColor, isNull);
        }
      },
    );

    test('arbitrary pixel spacing avoids the numeric scale', () {
      final stack =
          viewIrFromSource(
                'stack col gap-[2.5px] p-[6px] px-[10]\n  text "A"',
              ).root.single
              as StackNode;
      expect(stack.spacing, 2.5);
      expect(stack.style!.padding, 6);
      expect(stack.style!.paddingHorizontal, 10);
      for (final value in ['[Infinitypx]', '[NaN]', '[1e309px]', '[6rem]']) {
        final invalid =
            viewIrFromSource(
                  'stack col gap-$value p-$value\n  text "A"',
                ).root.single
                as StackNode;
        expect(invalid.spacing, isNull);
        expect(invalid.style?.padding, isNull);
      }
    });

    test('fractional spacing uses the shared finite scale', () {
      final ir = viewIrFromSource('stack col gap-0.5 p-1.5 px-2.5\n  text "A"');
      final stack = ir.root.single as StackNode;
      expect(stack.spacing, 2);
      expect(stack.style!.padding, 6);
      expect(stack.style!.paddingHorizontal, 10);
      final invalid =
          viewIrFromSource(
                'stack col gap-Infinity p-NaN\n  text "A"',
              ).root.single
              as StackNode;
      expect(invalid.spacing, isNull);
      expect(invalid.style?.padding, isNull);
    });

    test('quoted braces and nested action payloads remain whole tokens', () {
      for (final payload in [
        'prompt:"literal } with spaces"',
        'prompt:{"label":"a } b"}',
        "prompt:'literal } with spaces'",
      ]) {
        final ir = viewIrFromSource('button "Go" onclick={$payload}');
        expect((ir.root.single as ButtonNode).onClick, payload);
      }
    });

    test('nests children by indentation', () {
      final ir = viewIrFromSource('''
stack col gap-2
  text "a"
  stack row
    text "b"
    text "c"
''');
      expect(ir.root, hasLength(1));
      final outer = ir.root.single as StackNode;
      expect(outer.axis, StackAxis.column);
      expect(outer.spacing, 8);
      expect(outer.children, hasLength(2));
      expect(outer.children[0], isA<TextNode>());
      final inner = outer.children[1] as StackNode;
      expect(inner.axis, StackAxis.row);
      expect(inner.children, hasLength(2));
      expect((inner.children[0] as TextNode).content, 'b');
      expect((inner.children[1] as TextNode).content, 'c');
    });

    test('parses each allowlisted kind', () {
      final ir = viewIrFromSource('''
stack col
  text "hello"
  button "Press" onclick=complete
  toggle "Flag" checked onchange=complete
  checkbox "Ck" onchange=complete
  progress value=40 max=100
  meter value=3 min=0 max=5
  badge "New" tone=positive
  divider
  spacer size=12
  image src=https://example.com/a.png alt=Pic
  list
    li
      text "item"
  scroll
    text "scrolled"
''');
      final stack = ir.root.single as StackNode;
      final kinds = stack.children
          .map((n) => n.runtimeType.toString())
          .toList();
      expect(kinds, contains('TextNode'));
      expect(kinds, contains('ButtonNode'));
      expect(kinds, contains('ToggleNode'));
      expect(kinds, contains('CheckboxNode'));
      expect(kinds, contains('ProgressNode'));
      expect(kinds, contains('MeterNode'));
      expect(kinds, contains('BadgeNode'));
      expect(kinds, contains('DividerNode'));
      expect(kinds, contains('SpacerNode'));
      expect(kinds, contains('ImageNode'));
      expect(kinds, contains('ListNode'));
      expect(kinds, contains('ScrollNode'));

      final button = stack.children.whereType<ButtonNode>().single;
      expect(button.label, 'Press');
      expect(button.onClick, 'complete');
      final toggle = stack.children.whereType<ToggleNode>().single;
      expect(toggle.checked, isTrue);
      expect(toggle.onChange, 'complete');
      final progress = stack.children.whereType<ProgressNode>().single;
      expect(progress.value, 40);
      expect(progress.max, 100);
      final badge = stack.children.whereType<BadgeNode>().single;
      expect(badge.tone, 'positive');
      final image = stack.children.whereType<ImageNode>().single;
      expect(image.src, 'https://example.com/a.png');
      expect(image.alt, 'Pic');
    });

    test('disallowed tags decode to UnsupportedNode', () {
      final ir = viewIrFromSource('''
stack col
  webview src=https://evil.example
  input placeholder=x bind=y
  dropzone "drop"
  text "safe"
''');
      final stack = ir.root.single as StackNode;
      final unsupported = stack.children.whereType<UnsupportedNode>().toList();
      expect(
        unsupported.map((n) => n.kind),
        containsAll(['webview', 'input', 'dropzone']),
      );
      expect(stack.children.whereType<TextNode>().single.content, 'safe');
    });

    test('preserves action strings without interpreting them', () {
      final ir = viewIrFromSource(
        'button "Go" onclick={prompt:Draft the email to Sam}',
      );
      final button = ir.root.single as ButtonNode;
      expect(button.onClick, 'prompt:Draft the email to Sam');
    });

    test('oversized source is rejected to an empty document', () {
      final huge = 'text "x"\n' * 5000; // far beyond maxSourceLength
      final ir = viewIrFromSource(huge);
      expect(ir.root, isEmpty);
    });

    test('too many nodes is rejected', () {
      final buffer = StringBuffer('stack col\n');
      for (var i = 0; i < 100; i++) {
        buffer.writeln('  text "n$i"');
      }
      final ir = viewIrFromSource(buffer.toString());
      expect(ir.root, isEmpty);
    });

    test('too deep is rejected', () {
      final buffer = StringBuffer();
      for (var i = 0; i < 12; i++) {
        buffer.writeln('${'  ' * i}stack col');
      }
      final ir = viewIrFromSource(buffer.toString());
      expect(ir.root, isEmpty);
    });

    test('if / else parse into IfNode with both branches', () {
      final ir = viewIrFromSource('''
if done
  text "finished"
else
  text "pending"
''');
      final node = ir.root.single as IfNode;
      expect(node.condition, 'done');
      expect((node.thenChildren.single as TextNode).content, 'finished');
      expect((node.elseChildren!.single as TextNode).content, 'pending');
    });

    test('forEach parses bind + item name', () {
      final ir = viewIrFromSource('''
foreach items as row
  text "{row.title}"
''');
      final node = ir.root.single as ForEachNode;
      expect(node.bind, 'items');
      expect(node.itemName, 'row');
      expect((node.itemBody.single as TextNode).content, '{row.title}');
    });
  });

  group('ViewIr.fromJson', () {
    test('unknown kind becomes UnsupportedNode', () {
      final ir = ViewIr.fromJson({
        'version': 5,
        'root': [
          {'kind': 'webView', 'src': 'https://x'},
          {'kind': 'text', 'content': 'ok'},
        ],
      });
      expect(ir.root[0], isA<UnsupportedNode>());
      expect(ir.root[1], isA<TextNode>());
    });

    test('oversized IR is rejected', () {
      final root = List.generate(200, (i) => {'kind': 'text', 'content': '$i'});
      final ir = ViewIr.fromJson({'version': 5, 'root': root});
      expect(ir.root, isEmpty);
    });
  });
}
