import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('viewIrFromSource', () {
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
