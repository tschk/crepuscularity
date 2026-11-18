import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Widget _host(Widget child) => MaterialApp(home: Scaffold(body: child));

/// A structural fingerprint used to assert that `.crepus` source and the
/// equivalent hand-written View IR decode to the same tree.
Object _shape(ViewNode node) => switch (node) {
  TextNode(:final content) => {'text': content},
  StackNode(:final axis, :final spacing) => {
    'stack': axis.name,
    'spacing': spacing,
    'children': childrenOf(node).map(_shape).toList(),
  },
  ScrollNode(:final axis) => {
    'scroll': axis.name,
    'children': childrenOf(node).map(_shape).toList(),
  },
  ButtonNode(:final label, :final onClick) => {'button': label, 'on': onClick},
  TimerNode(:final label, :final duration, :final autostart) => {
    'timer': label,
    'duration': duration.inMilliseconds,
    'autostart': autostart,
  },
  ToggleNode(:final label, :final checked) => {
    'toggle': label,
    'checked': checked,
  },
  CheckboxNode(:final label, :final checked) => {
    'checkbox': label,
    'checked': checked,
  },
  ProgressNode(:final value, :final max) => {'progress': value, 'max': max},
  MeterNode(:final value, :final min, :final max) => {
    'meter': value,
    'min': min,
    'max': max,
  },
  SparklineNode(:final values, :final color, :final variant) => {
    'sparkline': values,
    'color': color,
    'variant': variant,
  },
  BadgeNode(:final label, :final tone) => {'badge': label, 'tone': tone},
  DividerNode(:final axis) => {'divider': axis.name},
  SpacerNode(:final size) => {'spacer': size},
  ImageNode(:final src, :final alt) => {'image': src, 'alt': alt},
  ListNode(:final ordered) => {
    'list': ordered,
    'children': childrenOf(node).map(_shape).toList(),
  },
  ListItemNode() => {
    'listItem': true,
    'children': childrenOf(node).map(_shape).toList(),
  },
  LinkNode(:final href, :final target, :final rel) => {
    'link': href,
    'target': target,
    'rel': rel,
    'children': childrenOf(node).map(_shape).toList(),
  },
  IfNode(:final condition) => {
    'if': condition,
    'children': childrenOf(node).map(_shape).toList(),
  },
  ForEachNode(:final bind, :final itemName) => {
    'forEach': bind,
    'item': itemName,
    'children': childrenOf(node).map(_shape).toList(),
  },
  UnsupportedNode(:final kind) => {'unsupported': kind},
};

List<Object> _shapeAll(List<ViewNode> nodes) =>
    nodes.map(_shape).toList(growable: false);

/// Assert that a `.crepus` document and an IR document decode identically, and
/// that both render the same visible text.
Future<void> _expectAgreement(
  WidgetTester tester,
  String source,
  List<Map<String, Object?>> root, {
  Map<String, Object?> data = const {},
  List<String> visible = const [],
}) async {
  final fromSource = viewIrFromSource(source);
  final fromIr = ViewIr.fromJson({'version': IR_VERSION, 'root': root});
  expect(_shapeAll(fromSource.root), _shapeAll(fromIr.root), reason: source);

  for (final view in [
    CrepusView.fromSource(source, data: data),
    CrepusView.fromIr({'version': IR_VERSION, 'root': root}, data: data),
  ]) {
    await tester.pumpWidget(_host(view));
    for (final label in visible) {
      expect(find.text(label), findsOneWidget, reason: '$label in $source');
    }
  }
}

void main() {
  testWidgets('a text document round trips', (tester) async {
    await _expectAgreement(
      tester,
      'text "Hello"',
      const [
        {'kind': 'text', 'content': 'Hello'},
      ],
      visible: const ['Hello'],
    );
  });

  testWidgets('a nested stack document round trips', (tester) async {
    await _expectAgreement(
      tester,
      '''
stack col gap-2
  text "top"
  stack row
    text "left"
    text "right"
''',
      const [
        {
          'kind': 'stack',
          'axis': 'column',
          'spacing': 8,
          'children': [
            {'kind': 'text', 'content': 'top'},
            {
              'kind': 'stack',
              'axis': 'row',
              'children': [
                {'kind': 'text', 'content': 'left'},
                {'kind': 'text', 'content': 'right'},
              ],
            },
          ],
        },
      ],
      visible: const ['top', 'left', 'right'],
    );
  });

  testWidgets('interactive leaves round trip with their actions', (
    tester,
  ) async {
    await _expectAgreement(
      tester,
      '''
stack col
  button "Do it" onclick=complete
  toggle "Flag" checked onchange=flip
  checkbox "Ck" onchange=tick
''',
      const [
        {
          'kind': 'stack',
          'axis': 'column',
          'children': [
            {'kind': 'button', 'label': 'Do it', 'onClick': 'complete'},
            {
              'kind': 'toggle',
              'label': 'Flag',
              'checked': true,
              'onChange': 'flip',
            },
            {'kind': 'checkbox', 'label': 'Ck', 'onChange': 'tick'},
          ],
        },
      ],
      visible: const ['Do it', 'Flag', 'Ck'],
    );
  });

  testWidgets('indicator leaves round trip', (tester) async {
    await _expectAgreement(
      tester,
      '''
stack col
  progress value=40 max=100
  meter value=3 min=0 max=5
  badge "New" tone=positive
  divider
  spacer size=12
  image src=https://example.com/a.png alt=Pic
''',
      const [
        {
          'kind': 'stack',
          'axis': 'column',
          'children': [
            {'kind': 'progress', 'value': 40, 'max': 100},
            {'kind': 'meter', 'value': 3, 'min': 0, 'max': 5},
            {'kind': 'badge', 'label': 'New', 'tone': 'positive'},
            {'kind': 'divider'},
            {'kind': 'spacer', 'size': 12},
            {'kind': 'image', 'src': 'https://example.com/a.png', 'alt': 'Pic'},
          ],
        },
      ],
      visible: const ['New'],
    );
  });

  testWidgets('lists round trip', (tester) async {
    await _expectAgreement(
      tester,
      '''
ol
  li
    text "one"
  li
    text "two"
''',
      const [
        {
          'kind': 'list',
          'ordered': true,
          'children': [
            {
              'kind': 'listItem',
              'children': [
                {'kind': 'text', 'content': 'one'},
              ],
            },
            {
              'kind': 'listItem',
              'children': [
                {'kind': 'text', 'content': 'two'},
              ],
            },
          ],
        },
      ],
      visible: const ['one', 'two', '1.', '2.'],
    );
  });

  testWidgets('scroll round trips', (tester) async {
    await _expectAgreement(
      tester,
      '''
scroll col
  text "inside"
''',
      const [
        {
          'kind': 'scroll',
          'axis': 'column',
          'children': [
            {'kind': 'text', 'content': 'inside'},
          ],
        },
      ],
      visible: const ['inside'],
    );
  });

  testWidgets('if/else round trips and picks the same branch', (tester) async {
    await _expectAgreement(
      tester,
      '''
if done
  text "finished"
else
  text "pending"
''',
      const [
        {
          'kind': 'if',
          'condition': 'done',
          'thenChildren': [
            {'kind': 'text', 'content': 'finished'},
          ],
          'elseChildren': [
            {'kind': 'text', 'content': 'pending'},
          ],
        },
      ],
      data: const {'done': true},
      visible: const ['finished'],
    );
  });

  testWidgets('forEach round trips and expands identically', (tester) async {
    await _expectAgreement(
      tester,
      '''
foreach items as row
  text "{row}"
''',
      const [
        {
          'kind': 'forEach',
          'bind': 'items',
          'itemName': 'row',
          'itemBody': [
            {'kind': 'text', 'content': '{row}'},
          ],
        },
      ],
      data: const {
        'items': ['alpha', 'beta'],
      },
      visible: const ['alpha', 'beta'],
    );
  });

  testWidgets('links round trip', (tester) async {
    await _expectAgreement(
      tester,
      '''
a href=https://example.com/docs target=_blank rel=noopener
  text "Read the docs"
''',
      const [
        {
          'kind': 'link',
          'href': 'https://example.com/docs',
          'target': '_blank',
          'rel': 'noopener',
          'children': [
            {'kind': 'text', 'content': 'Read the docs'},
          ],
        },
      ],
      visible: const ['Read the docs'],
    );
  });

  testWidgets('a link dispatches its href identically from source and IR', (
    tester,
  ) async {
    for (final build in <CrepusView Function(CrepusActionCallback)>[
      (onAction) => CrepusView.fromSource(
        'a "Docs" href=https://example.com/docs',
        onAction: onAction,
      ),
      (onAction) => CrepusView.fromIr({
        'version': IR_VERSION,
        'root': const [
          {
            'kind': 'link',
            'href': 'https://example.com/docs',
            'children': [
              {'kind': 'text', 'content': 'Docs'},
            ],
          },
        ],
      }, onAction: onAction),
    ]) {
      final actions = <String>[];
      await tester.pumpWidget(_host(build(actions.add)));
      await tester.tap(find.text('Docs'));
      expect(actions, ['https://example.com/docs']);
    }
  });

  testWidgets('a source #id survives into the IR and back out again', (
    tester,
  ) async {
    final parsed = viewIrFromSource('text "Titled" #hero text-lg');
    final style = parsed.root.single.style!;
    expect(style.id, 'hero');
    expect(style.classes, ['text-lg']);

    final wire = style.toJson();
    final decoded = ViewStyle.fromJson(wire)!;
    expect(decoded.id, 'hero');
    expect(decoded.classes, ['text-lg']);
    expect(decoded.fontSize, style.fontSize);

    final fromIr = ViewIr.fromJson({
      'version': IR_VERSION,
      'root': [
        {'kind': 'text', 'content': 'Titled', 'style': wire},
      ],
    });
    final irStyle = fromIr.root.single.style!;
    expect(irStyle.id, 'hero');
    expect(irStyle.classes, ['text-lg']);
    expect(irStyle.toJson(), wire);
  });

  testWidgets('a disallowed tag round trips as the same unsupported node', (
    tester,
  ) async {
    final fromSource = viewIrFromSource('webview src=https://evil.example');
    expect((fromSource.root.single as UnsupportedNode).kind, 'webview');
    final fromIr = ViewIr.fromJson(const {
      'version': 5,
      'root': [
        {'kind': 'webview', 'src': 'https://evil.example'},
      ],
    });
    expect(_shapeAll(fromSource.root), _shapeAll(fromIr.root));
  });

  testWidgets('a source document with styles renders the same as its IR', (
    tester,
  ) async {
    const source = 'text "Styled" text-xl font-bold text-red-500 p-2';
    final style = viewIrFromSource(source).root.single.style!;
    expect(style.fontSize, 20);
    expect(style.fontWeight, 700);
    expect(style.foregroundColor, '#ef4444');
    expect(style.padding, 8);

    await tester.pumpWidget(_host(CrepusView.fromSource(source)));
    final sourceText = tester.widget<Text>(find.text('Styled')).style!;

    await tester.pumpWidget(
      _host(
        CrepusView.fromIr(const {
          'version': 5,
          'root': [
            {
              'kind': 'text',
              'content': 'Styled',
              'style': {
                'fontSize': 20,
                'fontWeight': 700,
                'foregroundColor': '#ef4444',
                'padding': 8,
              },
            },
          ],
        }),
      ),
    );
    final irText = tester.widget<Text>(find.text('Styled')).style!;
    expect(sourceText.fontSize, irText.fontSize);
    expect(sourceText.fontWeight, irText.fontWeight);
    expect(sourceText.color, irText.color);
  });

  testWidgets('actions dispatch identically from source and from IR', (
    tester,
  ) async {
    for (final build in <CrepusView Function(CrepusActionCallback)>[
      (onAction) => CrepusView.fromSource(
        'button "Go" onclick={prompt:Draft it}',
        onAction: onAction,
      ),
      (onAction) => CrepusView.fromIr(const {
        'version': 5,
        'root': [
          {'kind': 'button', 'label': 'Go', 'onClick': 'prompt:Draft it'},
        ],
      }, onAction: onAction),
    ]) {
      final actions = <String>[];
      await tester.pumpWidget(_host(build(actions.add)));
      await tester.tap(find.text('Go'));
      expect(actions, ['prompt:Draft it']);
    }
  });

  testWidgets('a realistic model-authored card renders end to end', (
    tester,
  ) async {
    const source = '''
# A card of the sort a model would emit into omi's hub.
view p-4 gap-2
  h2 "Today" text-lg font-semibold
  if hasTasks
    ul
      foreach tasks as t
        li
          text "{t.title}"
  else
    p "Nothing due" text-sm
  divider
  stack row gap-2 justify-between
    badge "3 open" tone=warning
    button "Review" onclick={prompt:Review my open tasks}
''';
    final actions = <String>[];
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource(
          source,
          onAction: actions.add,
          data: const {
            'hasTasks': true,
            'tasks': [
              {'title': 'Ship the renderer'},
              {'title': 'Write the tests'},
            ],
          },
        ),
      ),
    );
    expect(find.text('Today'), findsOneWidget);
    expect(find.text('Ship the renderer'), findsOneWidget);
    expect(find.text('Write the tests'), findsOneWidget);
    expect(find.text('Nothing due'), findsNothing);
    expect(find.text('3 open'), findsOneWidget);
    await tester.tap(find.text('Review'));
    expect(actions, ['prompt:Review my open tasks']);
    expect(tester.takeException(), isNull);
  });

  testWidgets('the same card takes the else branch with different data', (
    tester,
  ) async {
    const source = '''
view
  if hasTasks
    text "some"
  else
    text "Nothing due"
''';
    await tester.pumpWidget(
      _host(CrepusView.fromSource(source, data: const {'hasTasks': false})),
    );
    expect(find.text('Nothing due'), findsOneWidget);
    expect(find.text('some'), findsNothing);
  });
}
