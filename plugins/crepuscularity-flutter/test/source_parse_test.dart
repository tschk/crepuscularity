import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter_test/flutter_test.dart';

/// Parser-level tests for `viewIrFromSource`. `.crepus` source is model-authored
/// too, so truncated, ragged and hostile documents must parse to *something*
/// inert rather than throwing.
void main() {
  test('quoted text preserves escaped backslashes before escape letters', () {
    final ir = viewIrFromSource(r'text "\\n \\t \\"');
    expect((ir.root.single as TextNode).content, r'\n \t \');
    final attr = viewIrFromSource(r'button label="\\n"');
    expect((attr.root.single as ButtonNode).label, r'\n');
    final escapes = viewIrFromSource(r'''text "\n\r\t\"\'\q"''');
    expect((escapes.root.single as TextNode).content, '\n\r\t"\'\\q');
  });

  group('lexing and trivia', () {
    test('an empty or whitespace-only source is an empty document', () {
      for (final source in const ['', '\n', '   ', '\n\n\t\n', '  \t  \n  ']) {
        final ir = viewIrFromSource(source);
        expect(ir.root, isEmpty, reason: '"$source"');
        expect(ir.version, IR_VERSION);
      }
    });

    test('comments and blank lines are skipped', () {
      final ir = viewIrFromSource('''
# a leading comment

text "kept"

  # an indented comment
''');
      expect(ir.root, hasLength(1));
      expect((ir.root.single as TextNode).content, 'kept');
    });

    test('frontmatter fences are skipped', () {
      final ir = viewIrFromSource('''
+++
title = "x"
+++
text "body"
''');
      expect(ir.root.whereType<TextNode>().last.content, 'body');
    });

    test('--- lines are skipped wherever they appear', () {
      final ir = viewIrFromSource('---\ntext "a"\n---\n');
      expect(ir.root, hasLength(1));
    });

    test('CRLF line endings do not leak into content', () {
      final ir = viewIrFromSource('stack col\r\n  text "a"\r\n');
      final stack = ir.root.single as StackNode;
      expect((stack.children.single as TextNode).content, 'a');
    });

    test('tabs count as indentation alongside spaces', () {
      final ir = viewIrFromSource('stack col\n\ttext "tabbed"\n');
      final stack = ir.root.single as StackNode;
      expect(stack.children, hasLength(1));
      expect((stack.children.single as TextNode).content, 'tabbed');
    });

    test('trailing whitespace does not create empty tokens', () {
      final ir = viewIrFromSource('text "a"   \t  \n');
      expect((ir.root.single as TextNode).content, 'a');
    });

    test('a source with no final newline still parses', () {
      final ir = viewIrFromSource('stack col\n  text "a"');
      expect((ir.root.single as StackNode).children, hasLength(1));
    });
  });

  group('malformed and truncated source', () {
    test('an unterminated string consumes to end of line without throwing', () {
      final ir = viewIrFromSource('text "unterminated');
      expect(ir.root, hasLength(1));
      expect((ir.root.single as TextNode).content, 'unterminated');
    });

    test('an unterminated brace expression is closed at end of line', () {
      final ir = viewIrFromSource('button "Go" onclick={prompt:do');
      final button = ir.root.single as ButtonNode;
      expect(button.onClick, isNotNull);
    });

    test('a truncated document parses the lines it has', () {
      final ir = viewIrFromSource('stack col gap-2\n  text "a"\n  stack row\n');
      final outer = ir.root.single as StackNode;
      expect(outer.children, hasLength(2));
      expect((outer.children[1] as StackNode).children, isEmpty);
    });

    test('an unindented continuation becomes a sibling, not a crash', () {
      final ir = viewIrFromSource('stack col\ntext "sibling"\n');
      expect(ir.root, hasLength(2));
      expect(ir.root[1], isA<TextNode>());
    });

    test('a dedent past the parent closes both levels', () {
      final ir = viewIrFromSource('''
stack col
  stack row
      text "deep"
  text "back"
''');
      final outer = ir.root.single as StackNode;
      expect(outer.children, hasLength(2));
      expect((outer.children[0] as StackNode).children, hasLength(1));
      expect((outer.children[1] as TextNode).content, 'back');
    });

    test('ragged, inconsistent indentation never throws', () {
      const ragged = '''
stack col
      text "a"
   text "b"
        text "c"
 text "d"
''';
      expect(() => viewIrFromSource(ragged), returnsNormally);
      expect(viewIrFromSource(ragged).root, isNotEmpty);
    });

    test('a leading-indented root line is still a root node', () {
      final ir = viewIrFromSource('    text "indented root"');
      expect((ir.root.single as TextNode).content, 'indented root');
    });

    test('a bare tag with nothing else becomes an empty stack', () {
      final ir = viewIrFromSource('div');
      final stack = ir.root.single as StackNode;
      expect(stack.children, isEmpty);
      expect(stack.axis, StackAxis.column);
    });

    test('garbage lines degrade to empty stacks rather than throwing', () {
      for (final source in const [
        '!!!',
        '=====',
        '=value',
        '{}{}{}',
        '""""',
        r'\\\\',
        '} unmatched {',
        'a=b=c=d',
      ]) {
        expect(() => viewIrFromSource(source), returnsNormally, reason: source);
      }
    });

    test('control characters in text are carried through as data', () {
      // NUL, BEL, an ANSI escape sequence and a bidi override. None of these
      // may terminate a token early or be silently stripped by the lexer.
      const hostile = 'a\u0000b\u0007c\u001b[31md\u202ee';
      final ir = viewIrFromSource('text "$hostile"');
      expect((ir.root.single as TextNode).content, hostile);
    });

    test('a newline escape inside text does not split the node', () {
      final ir = viewIrFromSource(r'text "line1\nline2"');
      expect(ir.root, hasLength(1));
      expect((ir.root.single as TextNode).content, 'line1\nline2');
    });

    test('non-ASCII tags and attributes do not throw', () {
      expect(() => viewIrFromSource('見出し "こんにちは"'), returnsNormally);
      expect(() => viewIrFromSource('text "日本語 🎉 emoji"'), returnsNormally);
      expect(
        (viewIrFromSource('text "日本語 🎉"').root.single as TextNode).content,
        '日本語 🎉',
      );
    });

    test('escape sequences are unescaped exactly once', () {
      final ir = viewIrFromSource(r'text "a\nb\tc\"d\\e"');
      expect((ir.root.single as TextNode).content, 'a\nb\tc"d\\e');
    });
  });

  group('tags', () {
    test('every text-ish tag maps to a TextNode', () {
      for (final tag in const [
        'text',
        'span',
        'p',
        'label',
        'caption',
        'h1',
        'h2',
        'h3',
        'h4',
        'h5',
        'h6',
      ]) {
        final ir = viewIrFromSource('$tag "hi"');
        expect(ir.root.single, isA<TextNode>(), reason: tag);
        expect((ir.root.single as TextNode).content, 'hi', reason: tag);
      }
    });

    test('tags are matched case-insensitively', () {
      expect(viewIrFromSource('TEXT "hi"').root.single, isA<TextNode>());
      expect(viewIrFromSource('Button "hi"').root.single, isA<ButtonNode>());
      expect(viewIrFromSource('WEBVIEW').root.single, isA<UnsupportedNode>());
    });

    test('a text tag with a nested string child picks it up', () {
      final ir = viewIrFromSource('span\n  "nested"\n');
      expect((ir.root.single as TextNode).content, 'nested');
    });

    test('a text tag with no content at all falls through to a stack', () {
      // `span` with neither inline nor child text has no content to render, so
      // it degrades to a container rather than an empty text node.
      expect(viewIrFromSource('span').root.single, isA<StackNode>());
    });

    test('toggle and switch are the same node', () {
      for (final tag in const ['toggle', 'switch']) {
        final node = viewIrFromSource('$tag "L" checked').root.single;
        expect(node, isA<ToggleNode>(), reason: tag);
        expect((node as ToggleNode).checked, isTrue, reason: tag);
      }
    });

    test('divider aliases and axis classes', () {
      expect(viewIrFromSource('hr').root.single, isA<DividerNode>());
      expect(
        (viewIrFromSource('divider row').root.single as DividerNode).axis,
        StackAxis.row,
      );
      expect(
        (viewIrFromSource('divider').root.single as DividerNode).axis,
        StackAxis.column,
      );
    });

    test('list aliases and ordering', () {
      expect((viewIrFromSource('ul').root.single as ListNode).ordered, isFalse);
      expect((viewIrFromSource('ol').root.single as ListNode).ordered, isTrue);
      expect(
        (viewIrFromSource('list').root.single as ListNode).ordered,
        isFalse,
      );
      expect(viewIrFromSource('flatlist').root.single, isA<ListNode>());
    });

    test('list item aliases', () {
      for (final tag in const ['li', 'list-item', 'listitem']) {
        expect(
          viewIrFromSource(tag).root.single,
          isA<ListItemNode>(),
          reason: tag,
        );
      }
    });

    test('listitem inline text and label become text children', () {
      final quoted =
          viewIrFromSource('list\n  listitem "Email from Sam"').root.single
              as ListNode;
      final quotedItem = quoted.children.single as ListItemNode;
      expect(quotedItem.children, hasLength(1));
      expect(
        (quotedItem.children.single as TextNode).content,
        'Email from Sam',
      );

      final labeled =
          viewIrFromSource('list\n  listitem label="Invoice"').root.single
              as ListNode;
      final labeledItem = labeled.children.single as ListItemNode;
      expect((labeledItem.children.single as TextNode).content, 'Invoice');
    });

    test('container aliases map to nested stacks', () {
      for (final tag in const ['stack', 'card', 'section', 'panel', 'group']) {
        final node = viewIrFromSource('$tag\n  text "child"').root.single;
        expect(node, isA<StackNode>(), reason: tag);
        expect((node as StackNode).children, hasLength(1));
      }
    });

    test('image aliases and attributes', () {
      for (final tag in const ['img', 'image']) {
        final node =
            viewIrFromSource(
                  '$tag src=https://e.com/a.png alt="A pic"',
                ).root.single
                as ImageNode;
        expect(node.src, 'https://e.com/a.png', reason: tag);
        expect(node.alt, 'A pic', reason: tag);
      }
    });

    test('an image with no src decodes to an empty src, not null', () {
      expect((viewIrFromSource('img').root.single as ImageNode).src, '');
    });

    test('scroll comes from the tag or an overflow class', () {
      expect(viewIrFromSource('scroll').root.single, isA<ScrollNode>());
      for (final cls in const [
        'overflow-scroll',
        'overflow-auto',
        'overflow-y-auto',
        'overflow-y-scroll',
        'scroll',
      ]) {
        expect(
          viewIrFromSource('div $cls').root.single,
          isA<ScrollNode>(),
          reason: cls,
        );
      }
    });

    test('an unknown tag becomes a stack container', () {
      final node = viewIrFromSource('section\n  text "a"\n').root.single;
      expect(node, isA<StackNode>());
      expect((node as StackNode).children, hasLength(1));
    });

    test('every explicitly disallowed tag becomes UnsupportedNode', () {
      const disallowed = [
        'iframe',
        'webview',
        'input',
        'textfield',
        'textinput',
        'textarea',
        'picker',
        'select',
        'slider',
        'tabs',
        'tabview',
        'page-switcher',
        'dropzone',
        'file-picker',
        'filepicker',
        'media-picker',
        'slot',
        'slot-rotate',
        'embed',
      ];
      for (final tag in disallowed) {
        final node = viewIrFromSource('$tag src=x onclick=go').root.single;
        expect(node, isA<UnsupportedNode>(), reason: tag);
        expect((node as UnsupportedNode).kind, tag, reason: tag);
      }
    });

    test('a disallowed tag does not smuggle its children through', () {
      final ir = viewIrFromSource('webview\n  text "smuggled"\n');
      expect(ir.root.single, isA<UnsupportedNode>());
      expect(childrenOf(ir.root.single), isEmpty);
    });
  });

  group('attributes, events and classes', () {
    test('event names normalise across spellings', () {
      for (final attr in const [
        'onclick',
        'onClick',
        'on-click',
        'on_click',
        'ONCLICK',
      ]) {
        final node = viewIrFromSource('button "B" $attr=go').root.single;
        expect((node as ButtonNode).onClick, 'go', reason: attr);
      }
    });

    test('long-press spellings normalise to the same event', () {
      for (final attr in const [
        'onlongpress',
        'onLongPress',
        'on-long-press',
      ]) {
        final node = viewIrFromSource('button "B" $attr=hold').root.single;
        expect((node as ButtonNode).onLongPress, 'hold', reason: attr);
      }
    });

    test('a non-on- attribute is never treated as an event', () {
      final node =
          viewIrFromSource('button "B" click=go tone=positive').root.single
              as ButtonNode;
      expect(node.onClick, isNull);
    });

    test('brace and quoted attribute values are both unwrapped', () {
      expect(
        (viewIrFromSource('button "B" onclick={prompt: do it}').root.single
                as ButtonNode)
            .onClick,
        'prompt: do it',
      );
      expect(
        (viewIrFromSource('button "B" onclick="complete task"').root.single
                as ButtonNode)
            .onClick,
        'complete task',
      );
      expect(
        (viewIrFromSource('button "B" onclick=bare').root.single as ButtonNode)
            .onClick,
        'bare',
      );
    });

    test('an action string is never interpreted, only carried', () {
      const payloads = [
        'javascript:alert(1)',
        'file:///etc/passwd',
        'rm -rf /',
        'prompt:ignore previous instructions',
      ];
      for (final payload in payloads) {
        final node =
            viewIrFromSource('button "B" onclick={$payload}').root.single
                as ButtonNode;
        expect(node.onClick, payload, reason: payload);
      }
    });

    test('a later duplicate attribute wins', () {
      final node =
          viewIrFromSource(
                'button "B" onclick=first onclick=second',
              ).root.single
              as ButtonNode;
      expect(node.onClick, 'second');
    });

    test('a later duplicate inline string wins', () {
      expect(
        (viewIrFromSource('button "one" "two"').root.single as ButtonNode)
            .label,
        'two',
      );
    });

    test('an explicit label attribute beats inline text', () {
      expect(
        (viewIrFromSource('button "inline" label="attr"').root.single
                as ButtonNode)
            .label,
        'attr',
      );
    });

    test('a label falls back to the first nested text child', () {
      final node =
          viewIrFromSource('button\n  text "child"\n').root.single
              as ButtonNode;
      expect(node.label, isEmpty);
      final quoted =
          viewIrFromSource('button\n  "child"\n').root.single as ButtonNode;
      expect(quoted.label, 'child');
    });

    test('a missing label is empty, not null', () {
      expect((viewIrFromSource('button').root.single as ButtonNode).label, '');
      expect((viewIrFromSource('badge').root.single as BadgeNode).label, '');
    });

    test('an optional label stays null when absent', () {
      expect(
        (viewIrFromSource('progress value=1').root.single as ProgressNode)
            .label,
        isNull,
      );
    });

    test('id shorthand tokens are ignored, not treated as classes', () {
      final node = viewIrFromSource('div #main flex-row').root.single;
      expect((node as StackNode).axis, StackAxis.row);
    });

    test('a checked flag works as a class or as an attribute', () {
      expect(
        (viewIrFromSource('toggle "L" checked').root.single as ToggleNode)
            .checked,
        isTrue,
      );
      expect(
        (viewIrFromSource('toggle "L" checked=true').root.single as ToggleNode)
            .checked,
        isTrue,
      );
      // An explicitly falsy value is honored (matching the IR decoder, which
      // treats anything other than JSON `true` as unchecked).
      for (final falsy in const ['false', 'FALSE', '0', 'no', 'off', '""']) {
        expect(
          (viewIrFromSource('toggle "L" checked=$falsy').root.single
                  as ToggleNode)
              .checked,
          isFalse,
          reason: falsy,
        );
      }
      // Any other value still counts as set.
      expect(
        (viewIrFromSource('toggle "L" checked=yes').root.single as ToggleNode)
            .checked,
        isTrue,
      );
      expect(
        (viewIrFromSource('toggle "L"').root.single as ToggleNode).checked,
        isFalse,
      );
    });

    test('numeric attributes reject junk and non-finite values', () {
      final junk =
          viewIrFromSource('progress value=lots max=nan').root.single
              as ProgressNode;
      expect(junk.value, 0);
      expect(junk.max, 100);
      final meter =
          viewIrFromSource('meter value=-5 min=-10 max=10').root.single
              as MeterNode;
      expect(meter.value, -5);
      expect(meter.min, -10);
      expect(meter.max, 10);
    });

    test('spacer size comes from an attribute or a gap class', () {
      expect(
        (viewIrFromSource('spacer size=20').root.single as SpacerNode).size,
        20,
      );
      expect(
        (viewIrFromSource('spacer gap-3').root.single as SpacerNode).size,
        12,
      );
      expect(
        (viewIrFromSource('spacer').root.single as SpacerNode).size,
        isNull,
      );
    });
  });

  group('layout classes', () {
    test('last explicit direction wins without bare flex overriding it', () {
      for (final (classes, axis) in [
        ('flex-col flex-row', StackAxis.row),
        ('flex-row flex-col', StackAxis.column),
        ('row column', StackAxis.column),
        ('column row', StackAxis.row),
        ('flex-col flex', StackAxis.column),
        ('flex flex-col', StackAxis.column),
      ]) {
        expect(
          (viewIrFromSource('div $classes').root.single as StackNode).axis,
          axis,
          reason: classes,
        );
      }
    });

    test('axis classes select row or column', () {
      for (final cls in const ['flex-col', 'col', 'column']) {
        expect(
          (viewIrFromSource('div $cls').root.single as StackNode).axis,
          StackAxis.column,
          reason: cls,
        );
      }
      for (final cls in const ['flex-row', 'row', 'flex']) {
        expect(
          (viewIrFromSource('div $cls').root.single as StackNode).axis,
          StackAxis.row,
          reason: cls,
        );
      }
      expect(
        (viewIrFromSource('div').root.single as StackNode).axis,
        StackAxis.column,
      );
    });

    test('a `view` tag is always a column regardless of classes', () {
      expect(
        (viewIrFromSource('view flex-row').root.single as StackNode).axis,
        StackAxis.column,
      );
    });

    test('gap classes scale by 4 and reject non-integers', () {
      expect(
        (viewIrFromSource('div gap-0').root.single as StackNode).spacing,
        0,
      );
      expect(
        (viewIrFromSource('div gap-6').root.single as StackNode).spacing,
        24,
      );
      expect(
        (viewIrFromSource('div gap-huge').root.single as StackNode).spacing,
        isNull,
      );
    });

    test('alignment and justification classes map to hints', () {
      const align = {
        'items-center': 'center',
        'items-end': 'end',
        'items-start': 'start',
        'items-stretch': 'stretch',
      };
      align.forEach((cls, expected) {
        expect(
          (viewIrFromSource('div $cls').root.single as StackNode).alignItems,
          expected,
          reason: cls,
        );
      });
      const justify = {
        'justify-center': 'center',
        'justify-end': 'end',
        'justify-between': 'between',
        'justify-around': 'around',
        'justify-evenly': 'evenly',
        'justify-start': 'start',
      };
      justify.forEach((cls, expected) {
        expect(
          (viewIrFromSource('div $cls').root.single as StackNode)
              .justifyContent,
          expected,
          reason: cls,
        );
      });
      final bare = viewIrFromSource('div').root.single as StackNode;
      expect(bare.alignItems, isNull);
      expect(bare.justifyContent, isNull);
    });
  });

  group('style classes', () {
    ViewStyle? styleOf(String classes) =>
        viewIrFromSource('text "x" $classes').root.single.style;

    test('unknown classes resolve to no hints but are preserved verbatim', () {
      final style = styleOf('not-a-class weird-thing')!;
      expect(style.classes, ['not-a-class', 'weird-thing']);
      expect(style.fontSize, isNull);
      expect(style.padding, isNull);
      expect(style.foregroundColor, isNull);
      expect(style.cornerRadius, isNull);
    });

    test('font size classes map to point sizes', () {
      const cases = {
        'text-xs': 12.0,
        'text-sm': 14.0,
        'text-base': 16.0,
        'text-lg': 18.0,
        'text-xl': 20.0,
        'text-2xl': 24.0,
        'text-3xl': 30.0,
      };
      cases.forEach((cls, size) {
        expect(styleOf(cls)!.fontSize, size, reason: cls);
      });
    });

    test('font weight classes map to numeric weights', () {
      const cases = {
        'font-thin': 100,
        'font-light': 300,
        'font-normal': 400,
        'font-medium': 500,
        'font-semibold': 600,
        'font-bold': 700,
        'font-extrabold': 800,
        'font-black': 900,
      };
      cases.forEach((cls, weight) {
        expect(styleOf(cls)!.fontWeight, weight, reason: cls);
      });
    });

    test('decoration classes set their flags', () {
      expect(styleOf('italic')!.italic, isTrue);
      expect(styleOf('underline')!.underline, isTrue);
      expect(styleOf('line-through')!.strikethrough, isTrue);
    });

    test('text alignment classes', () {
      expect(styleOf('text-left')!.textAlign, 'left');
      expect(styleOf('text-center')!.textAlign, 'center');
      expect(styleOf('text-right')!.textAlign, 'right');
    });

    test('padding classes scale by 4 across every axis', () {
      final style = styleOf('p-1 px-2 py-3 pt-4 pb-5 pl-6 pr-7')!;
      expect(style.padding, 4);
      expect(style.paddingHorizontal, 8);
      expect(style.paddingVertical, 12);
      expect(style.paddingTop, 16);
      expect(style.paddingBottom, 20);
      expect(style.paddingLeft, 24);
      expect(style.paddingRight, 28);
    });

    test('a non-numeric padding class is ignored', () {
      expect(styleOf('p-lots')!.padding, isNull);
    });

    test('rounded classes map to radii', () {
      const cases = {
        'rounded-none': 0.0,
        'rounded-sm': 4.0,
        'rounded': 8.0,
        'rounded-md': 10.0,
        'rounded-lg': 12.0,
        'rounded-xl': 16.0,
        'rounded-2xl': 20.0,
        'rounded-full': 999.0,
        'rounded-bogus': null,
      };
      cases.forEach((cls, radius) {
        expect(styleOf(cls)!.cornerRadius, radius, reason: cls);
      });
    });

    test('only palette colour names are recognised', () {
      expect(styleOf('text-red-500')!.foregroundColor, '#fb2c36');
      expect(styleOf('bg-blue-900')!.backgroundColor, '#1c398e');
      expect(styleOf('text-white')!.foregroundColor, '#ffffff');
      expect(styleOf('bg-transparent')!.backgroundColor, '#00000000');
      // Arbitrary values are not honoured — no `text-[#deadbe]` escape hatch.
      expect(styleOf('text-#ff0000')!.foregroundColor, isNull);
      expect(styleOf('bg-hotpink')!.backgroundColor, isNull);
      expect(styleOf('text-red-501')!.foregroundColor, isNull);
    });
  });

  group('source identity', () {
    ViewStyle? styleOf(String source) =>
        viewIrFromSource(source).root.single.style;

    test('the #id shorthand is preserved on every tag that carries style', () {
      const cases = {
        'text "x" #hero': 'hero',
        'button "Go" #cta onclick=go': 'cta',
        'stack col #wrap': 'wrap',
        'badge "New" #chip': 'chip',
        'img src=a.png #pic': 'pic',
        'ul #items': 'items',
        'foreach rows as r #loop': 'loop',
      };
      cases.forEach((source, id) {
        expect(styleOf(source)!.id, id, reason: source);
      });
    });

    test('a bare # with no name leaves the id unset', () {
      expect(styleOf('text "x" # p-2')!.id, isNull);
    });

    test('raw classes are carried alongside the resolved hints', () {
      final style = styleOf('text "x" p-2 text-lg mystery')!;
      expect(style.classes, ['p-2', 'text-lg', 'mystery']);
      expect(style.padding, 8);
      expect(style.fontSize, 18);
    });

    test('classes exclude attributes, events, ids and inline text', () {
      final style = styleOf('button "Go" #cta p-2 onclick=go label=Other')!;
      expect(style.classes, ['p-2']);
      expect(style.id, 'cta');
    });

    test('a tag with no classes and no id has no style at all', () {
      expect(styleOf('text "x"'), isNull);
    });
  });

  group('link', () {
    LinkNode linkOf(String source) =>
        viewIrFromSource(source).root.single as LinkNode;

    test('both the `a` and `link` spellings parse', () {
      for (final tag in const ['a', 'link']) {
        final node = linkOf('$tag "Docs" href=https://example.com/docs');
        expect(node.href, 'https://example.com/docs', reason: tag);
        expect((node.children.single as TextNode).content, 'Docs', reason: tag);
      }
    });

    test('target and rel are carried through', () {
      final node = linkOf('a "X" href=/x target=_blank rel=noopener');
      expect(node.target, '_blank');
      expect(node.rel, 'noopener');
    });

    test('a link with no href parses with an empty href', () {
      final node = linkOf('a "Dead"');
      expect(node.href, '');
      expect(node.target, isNull);
      expect(node.rel, isNull);
    });

    test('indented children win over inline text', () {
      final node = linkOf('''
a href=https://example.com "ignored"
  text "one"
  text "two"
''');
      expect(node.children, hasLength(2));
      expect((node.children.first as TextNode).content, 'one');
    });

    test('a link nests containers like any other block', () {
      final node = linkOf('''
a href=https://example.com p-2
  stack row gap-2
    text "left"
    badge "right"
''');
      expect(node.style!.padding, 8);
      expect(node.children.single, isA<StackNode>());
      expect(childrenOf(node), hasLength(1));
    });
  });

  group('control flow', () {
    test('if without an else has a null else branch', () {
      final node =
          viewIrFromSource('if done\n  text "a"\n').root.single as IfNode;
      expect(node.condition, 'done');
      expect(node.elseChildren, isNull);
    });

    test('a multi-token condition is rejoined verbatim', () {
      final node =
          viewIrFromSource('if status == "done"\n  text "a"\n').root.single
              as IfNode;
      expect(node.condition, 'status == "done"');
    });

    test('an if with no condition parses to an empty condition', () {
      expect(
        (viewIrFromSource('if\n  text "a"\n').root.single as IfNode).condition,
        '',
      );
    });

    test('a stray else with no preceding if is inert', () {
      final ir = viewIrFromSource('else\n  text "orphan"\n');
      expect(ir.root, isEmpty);
    });

    test('only an immediately-following else attaches to an if', () {
      final ir = viewIrFromSource('''
if a
  text "then"
text "between"
else
  text "orphaned"
''');
      final ifNode = ir.root.first as IfNode;
      expect(ifNode.elseChildren, isNull);
      expect(ir.root, hasLength(2));
    });

    test('consecutive ifs each keep their own else', () {
      final ir = viewIrFromSource('''
if a
  text "A"
else
  text "notA"
if b
  text "B"
else
  text "notB"
''');
      expect(ir.root, hasLength(2));
      final first = ir.root[0] as IfNode;
      final second = ir.root[1] as IfNode;
      expect((first.elseChildren!.single as TextNode).content, 'notA');
      expect((second.elseChildren!.single as TextNode).content, 'notB');
    });

    test('a nested if inside an if branch parses', () {
      final ir = viewIrFromSource('''
if a
  if b
    text "both"
  else
    text "onlyA"
''');
      final outer = ir.root.single as IfNode;
      final inner = outer.thenChildren.single as IfNode;
      expect(inner.condition, 'b');
      expect((inner.elseChildren!.single as TextNode).content, 'onlyA');
    });

    test('`for item in list` binds the item name and the list', () {
      final node =
          viewIrFromSource('for row in items\n  text "{row}"\n').root.single
              as ForEachNode;
      expect(node.bind, 'items');
      expect(node.itemName, 'row');
    });

    test('`foreach list as item` binds in the other order', () {
      final node =
          viewIrFromSource('foreach items as row\n  text "{row}"\n').root.single
              as ForEachNode;
      expect(node.bind, 'items');
      expect(node.itemName, 'row');
    });

    test('a single-token loop binds the list with the default item name', () {
      final node =
          viewIrFromSource('for items\n  text "x"\n').root.single
              as ForEachNode;
      expect(node.bind, 'items');
      expect(node.itemName, 'item');
    });

    test('a loop with no tokens at all is inert but well-formed', () {
      final node =
          viewIrFromSource('foreach\n  text "x"\n').root.single as ForEachNode;
      expect(node.bind, '');
      expect(node.itemName, 'item');
    });

    test('a malformed loop header does not throw', () {
      for (final header in const [
        'for in',
        'for in items',
        'foreach as',
        'for as in',
      ]) {
        expect(
          () => viewIrFromSource('$header\n  text "x"\n'),
          returnsNormally,
          reason: header,
        );
      }
    });

    test('interpolation is deferred to the renderer, never resolved here', () {
      final node =
          viewIrFromSource('text "Hello {name}"').root.single as TextNode;
      expect(node.content, 'Hello {name}');
    });
  });

  group('limits', () {
    test('source exactly at the length cap is accepted', () {
      const line = 'text "x"\n';
      final source = line * (8000 ~/ line.length);
      expect(source.length, lessThanOrEqualTo(8000));
      final ir = viewIrFromSource(
        source,
        limits: const CrepusLimits(maxNodes: 100000),
      );
      expect(ir.root, isNotEmpty);
    });

    test('source one character past the cap is rejected', () {
      final source = 'x' * 8001;
      expect(viewIrFromSource(source).root, isEmpty);
    });

    test('custom source-length limits are honoured', () {
      expect(
        viewIrFromSource(
          'text "hello there"',
          limits: const CrepusLimits(maxSourceLength: 5),
        ).root,
        isEmpty,
      );
    });

    test('the node cap applies to parsed source too', () {
      final buffer = StringBuffer();
      for (var i = 0; i < 60; i++) {
        buffer.writeln('text "n$i"');
      }
      expect(viewIrFromSource(buffer.toString()).root, hasLength(60));
      buffer.writeln('text "one too many"');
      expect(viewIrFromSource(buffer.toString()).root, isEmpty);
    });

    test('the depth cap applies to parsed source too', () {
      String nested(int depth) {
        final buffer = StringBuffer();
        for (var i = 0; i < depth; i++) {
          buffer.writeln('${'  ' * i}div');
        }
        return buffer.toString();
      }

      expect(viewIrFromSource(nested(8)).root, isNotEmpty);
      expect(viewIrFromSource(nested(9)).root, isEmpty);
    });

    test('a version is always stamped, even on a rejected document', () {
      expect(viewIrFromSource('x' * 9000).version, IR_VERSION);
      expect(viewIrFromSource('').version, IR_VERSION);
      expect(IR_VERSION, 7);
    });
  });
}
