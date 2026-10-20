import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Widget _host(Widget child) => MaterialApp(home: Scaffold(body: child));

/// Render a single-node document straight from IR, bypassing the parser so the
/// renderer's own handling of each kind is what is under test.
Widget _one(
  Map<String, Object?> node, {
  CrepusActionCallback? onAction,
  Map<String, Object?> data = const {},
  bool allowNetworkImages = false,
}) => _host(
  CrepusView.fromIr(
    {
      'version': 5,
      'root': [node],
    },
    onAction: onAction,
    data: data,
    allowNetworkImages: allowNetworkImages,
  ),
);

void main() {
  testWidgets(
    'invalid tracking values preserve valid source and drop invalid IR',
    (tester) async {
      for (final token in ['NaNpx', 'Infinitypx', '1e309px', '6rem']) {
        await tester.pumpWidget(
          _host(
            CrepusView.fromSource(
              'text tracking-wide tracking-[$token] "Tracked"',
            ),
          ),
        );
        expect(
          tester.widget<Text>(find.text('Tracked')).style!.letterSpacing,
          0.4,
        );
      }
      for (final value in [
        double.nan,
        double.infinity,
        double.negativeInfinity,
      ]) {
        final style = ViewStyle.fromJson({'letterSpacing': value});
        expect(style, isNull);
        await tester.pumpWidget(
          _one({
            'kind': 'text',
            'content': 'Tracked',
            'style': {'letterSpacing': value},
          }),
        );
        expect(
          tester.widget<Text>(find.text('Tracked')).style!.letterSpacing,
          isNull,
        );
        expect(tester.takeException(), isNull);
      }
    },
  );

  testWidgets('letter spacing survives source and IR rendering', (
    tester,
  ) async {
    for (final (classes, spacing) in [
      ('tracking-tight', -0.4),
      ('tracking-wide', 0.4),
      ('tracking-[2.5px]', 2.5),
    ]) {
      final style =
          (viewIrFromSource('text $classes "Tracked"').root.single as TextNode)
              .style!;
      expect(style.letterSpacing, spacing);
      expect(ViewStyle.fromJson(style.toJson())!.letterSpacing, spacing);
      for (final view in [
        CrepusView.fromSource('text $classes "Tracked"'),
        CrepusView.fromIr({
          'version': IR_VERSION,
          'root': [
            {
              'kind': 'text',
              'content': 'Tracked',
              'style': {'letterSpacing': spacing},
            },
          ],
        }),
      ]) {
        await tester.pumpWidget(_host(view));
        expect(
          tester.widget<Text>(find.text('Tracked')).style!.letterSpacing,
          spacing,
        );
      }
    }
  });

  testWidgets('source font weights reach every supported Flutter weight', (
    tester,
  ) async {
    final classes = [
      'thin',
      'extralight',
      'light',
      'normal',
      'medium',
      'semibold',
      'bold',
      'extrabold',
      'black',
    ];
    for (var index = 0; index < classes.length; index++) {
      await tester.pumpWidget(
        _host(CrepusView.fromSource('text font-${classes[index]} "Weight"')),
      );
      expect(
        tester.widget<Text>(find.text('Weight')).style!.fontWeight,
        FontWeight.values[index],
      );
    }
  });

  testWidgets('source direction order changes the rendered stack', (
    tester,
  ) async {
    for (final (classes, row) in [
      ('flex-col flex-row', true),
      ('flex-row flex-col', false),
    ]) {
      await tester.pumpWidget(
        _host(CrepusView.fromSource('div $classes\n  text "Direction"')),
      );
      final ancestors = find.ancestor(
        of: find.text('Direction'),
        matching: find.byType(Row),
      );
      expect(ancestors, row ? findsOneWidget : findsNothing);
      expect(tester.takeException(), isNull);
    }
  });

  testWidgets('source evenly justification reaches row layout', (tester) async {
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource(
          'stack row justify-start justify-evenly\n  text "Even"',
        ),
      ),
    );
    final row = find.ancestor(
      of: find.text('Even'),
      matching: find.byType(Row),
    );
    expect(
      tester.widget<Row>(row).mainAxisAlignment,
      MainAxisAlignment.spaceEvenly,
    );
  });

  testWidgets('source layout uses the last alignment classes', (tester) async {
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource(
          'stack row items-center items-end justify-center justify-start\n  text "Aligned"',
        ),
      ),
    );
    final row = find.ancestor(
      of: find.text('Aligned'),
      matching: find.byType(Row),
    );
    expect(row, findsOneWidget);
    expect(tester.widget<Row>(row).crossAxisAlignment, CrossAxisAlignment.end);
    expect(tester.widget<Row>(row).mainAxisAlignment, MainAxisAlignment.start);
  });

  testWidgets('source opacity reaches the rendered widget', (tester) async {
    await tester.pumpWidget(
      _host(CrepusView.fromSource('text opacity-25 "Faded"')),
    );
    final opacity = find.ancestor(
      of: find.text('Faded'),
      matching: find.byType(Opacity),
    );
    expect(opacity, findsOneWidget);
    expect(tester.widget<Opacity>(opacity).opacity, 0.25);
  });

  testWidgets('source border width uses the theme color by default', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource(
          'text border-2 "Default border"',
          theme: const CrepusTheme(borderColor: Colors.green),
        ),
      ),
    );
    final decorations = tester
        .widgetList<DecoratedBox>(find.byType(DecoratedBox))
        .map((widget) => widget.decoration)
        .whereType<BoxDecoration>();
    expect(
      decorations.any(
        (decoration) =>
            decoration.border?.top.width == 2 &&
            decoration.border?.top.color == Colors.green,
      ),
      isTrue,
    );
  });

  testWidgets('source border classes reach rendered decoration', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(CrepusView.fromSource('text border-2 border-[#f008] "Border"')),
    );
    final decorations = tester
        .widgetList<DecoratedBox>(find.byType(DecoratedBox))
        .map((widget) => widget.decoration)
        .whereType<BoxDecoration>();
    expect(
      decorations.any(
        (decoration) =>
            decoration.border?.top.width == 2 &&
            decoration.border?.top.color == const Color(0x88ff0000),
      ),
      isTrue,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('arbitrary source font size reaches rendered text', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(CrepusView.fromSource('text text-[18.5px] "Font"')),
    );
    expect(tester.widget<Text>(find.text('Font')).style!.fontSize, 18.5);
    expect(tester.takeException(), isNull);
  });

  testWidgets('arbitrary source radius reaches rendered decoration', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(CrepusView.fromSource('text rounded-[6px] bg-[#f00] "Radius"')),
    );
    final decorations = tester
        .widgetList<DecoratedBox>(find.byType(DecoratedBox))
        .map((widget) => widget.decoration)
        .whereType<BoxDecoration>();
    expect(
      decorations.any(
        (decoration) => decoration.borderRadius == BorderRadius.circular(6),
      ),
      isTrue,
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('CSS shorthand colors agree between source and IR', (
    tester,
  ) async {
    for (final (hex, expected) in [
      ('#f00', const Color(0xffff0000)),
      ('#f008', const Color(0x88ff0000)),
    ]) {
      for (final source in [false, true]) {
        await tester.pumpWidget(
          source
              ? _host(CrepusView.fromSource('text text-[$hex] "Short"'))
              : _one({
                  'kind': 'text',
                  'content': 'Short',
                  'style': {'foregroundColor': hex},
                }),
        );
        expect(tester.widget<Text>(find.text('Short')).style!.color, expected);
        expect(tester.takeException(), isNull);
      }
    }
  });

  testWidgets('arbitrary source foreground color renders CSS alpha', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(CrepusView.fromSource('text text-[#ff000080] "Hex"')),
    );
    expect(
      tester.widget<Text>(find.text('Hex')).style!.color,
      const Color(0x80ff0000),
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('arbitrary source gap renders exact pixels', (tester) async {
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource('stack col gap-[2.5px]\n  text "A"\n  text "B"'),
      ),
    );
    expect(
      tester.getRect(find.text('B')).top -
          tester.getRect(find.text('A')).bottom,
      closeTo(2.5, 0.001),
    );
    expect(tester.takeException(), isNull);
  });

  testWidgets('fractional source gaps produce fractional-scale layout', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource(
          'stack col gap-0.5 p-1.5\n  text "A"\n  text "B"',
        ),
      ),
    );
    final first = tester.getRect(find.text('A'));
    final second = tester.getRect(find.text('B'));
    expect(second.top - first.bottom, closeTo(2, 0.001));
    expect(tester.takeException(), isNull);
  });

  testWidgets('source action payload preserves quoted braces through taps', (
    tester,
  ) async {
    const payload = 'prompt:{"label":"a } b"}';
    final actions = <String>[];
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource(
          'button "Go" onclick={$payload}',
          onAction: actions.add,
        ),
      ),
    );
    await tester.tap(find.text('Go'));
    expect(actions, [payload]);
    expect(tester.takeException(), isNull);
  });

  testWidgets('bars clamp extreme values and preserve tiny ranges', (
    tester,
  ) async {
    for (final (node, expected) in [
      ({'kind': 'progress', 'value': 1e308, 'max': 5e-324}, 1.0),
      ({'kind': 'progress', 'value': -1e308, 'max': 5e-324}, 0.0),
      ({'kind': 'meter', 'value': 0, 'min': -5e-324, 'max': 5e-324}, 0.5),
      ({'kind': 'meter', 'value': 1e308, 'min': -1e308, 'max': 1e308}, 1.0),
    ]) {
      await tester.pumpWidget(_one(node));
      expect(tester.takeException(), isNull, reason: '$node');
      expect(
        tester
            .widget<LinearProgressIndicator>(
              find.byType(LinearProgressIndicator),
            )
            .value,
        expected,
        reason: '$node',
      );
    }
  });

  testWidgets('signed hex colors fall back to the theme', (tester) async {
    await tester.pumpWidget(_one(const {'kind': 'text', 'content': 'Color'}));
    final fallback = tester.widget<Text>(find.text('Color')).style!.color;
    for (final hex in ['#+1234567', '#-1234567']) {
      await tester.pumpWidget(
        _one({
          'kind': 'text',
          'content': 'Color',
          'style': {'foregroundColor': hex},
        }),
      );
      expect(
        tester.widget<Text>(find.text('Color')).style!.color,
        fallback,
        reason: hex,
      );
    }
  });

  testWidgets('meter midpoint survives finite range overflow', (tester) async {
    await tester.pumpWidget(
      _one(const {'kind': 'meter', 'value': 0, 'min': -1e308, 'max': 1e308}),
    );
    expect(find.text('50%'), findsOneWidget);
    expect(
      tester
          .widget<LinearProgressIndicator>(find.byType(LinearProgressIndicator))
          .value,
      0.5,
    );
  });

  testWidgets('CSS alpha color ignores surrounding whitespace', (tester) async {
    await tester.pumpWidget(
      _one(const {
        'kind': 'text',
        'content': 'Alpha',
        'style': {'foregroundColor': ' #ff000080 '},
      }),
    );
    final text = tester.widget<Text>(find.text('Alpha'));
    expect(text.style!.color, const Color(0x80ff0000));
  });

  group('text', () {
    testWidgets('renders content and honors style', (tester) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'text',
          'content': 'Hello',
          'style': {
            'fontSize': 22,
            'fontWeight': 700,
            'textAlign': 'center',
            'foregroundColor': '#ff0000',
            'italic': true,
            'underline': true,
            'strikethrough': true,
          },
        }),
      );
      final text = tester.widget<Text>(find.text('Hello'));
      expect(text.style!.fontSize, 22);
      expect(text.style!.fontWeight, FontWeight.w700);
      expect(text.style!.color, const Color(0xffff0000));
      expect(text.style!.fontStyle, FontStyle.italic);
      expect(text.style!.decoration, isNotNull);
      expect(text.textAlign, TextAlign.center);
    });

    testWidgets('empty content renders nothing', (tester) async {
      await tester.pumpWidget(_one(const {'kind': 'text', 'content': ''}));
      expect(find.byType(Text), findsNothing);
    });

    testWidgets('unresolved interpolation collapses to empty', (tester) async {
      await tester.pumpWidget(
        _one(const {'kind': 'text', 'content': 'A{missing}B'}),
      );
      expect(find.text('AB'), findsOneWidget);
    });

    testWidgets('dotted paths resolve through nested maps', (tester) async {
      await tester.pumpWidget(
        _one(
          const {'kind': 'text', 'content': '{user.name} / {user.a.b}'},
          data: const {
            'user': {
              'name': 'Ada',
              'a': {'b': 7},
            },
          },
        ),
      );
      expect(find.text('Ada / 7'), findsOneWidget);
    });

    testWidgets('a path through a non-map resolves to empty', (tester) async {
      await tester.pumpWidget(
        _one(
          const {'kind': 'text', 'content': '[{name.oops}]'},
          data: const {'name': 'Ada'},
        ),
      );
      expect(find.text('[]'), findsOneWidget);
    });

    testWidgets('an unterminated brace is left verbatim', (tester) async {
      await tester.pumpWidget(
        _one(const {'kind': 'text', 'content': 'a {unclosed'}),
      );
      expect(find.text('a {unclosed'), findsOneWidget);
    });

    testWidgets('control characters and RTL overrides render as data', (
      tester,
    ) async {
      const hostile = 'a\u0000b\u0007c\u001b[31md\u202egnp.exe';
      await tester.pumpWidget(_one(const {'kind': 'text', 'content': hostile}));
      expect(find.text(hostile), findsOneWidget);
    });

    testWidgets('astral-plane and combining characters survive', (
      tester,
    ) async {
      const emoji = '👩‍👩‍👧 é́ 中文';
      await tester.pumpWidget(_one(const {'kind': 'text', 'content': emoji}));
      expect(find.text(emoji), findsOneWidget);
    });

    testWidgets('a very long string renders without throwing', (tester) async {
      final long = 'x' * 20000;
      await tester.pumpWidget(
        _host(
          SingleChildScrollView(
            child: CrepusView.fromIr({
              'version': 5,
              'root': [
                {'kind': 'text', 'content': long},
              ],
            }),
          ),
        ),
      );
      expect(find.text(long), findsOneWidget);
      expect(tester.takeException(), isNull);
    });
  });

  group('stack', () {
    testWidgets('column is the default axis and children render in order', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'stack',
          'children': [
            {'kind': 'text', 'content': 'one'},
            {'kind': 'text', 'content': 'two'},
          ],
        }),
      );
      expect(find.byType(Column), findsWidgets);
      expect(find.text('one'), findsOneWidget);
      expect(find.text('two'), findsOneWidget);
      expect(
        tester.getTopLeft(find.text('one')).dy,
        lessThan(tester.getTopLeft(find.text('two')).dy),
      );
    });

    testWidgets('row lays children out horizontally', (tester) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'stack',
          'axis': 'row',
          'children': [
            {'kind': 'text', 'content': 'L'},
            {'kind': 'text', 'content': 'R'},
          ],
        }),
      );
      expect(
        tester.getTopLeft(find.text('L')).dx,
        lessThan(tester.getTopLeft(find.text('R')).dx),
      );
    });

    testWidgets('spacing inserts a gap between siblings only', (tester) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'stack',
          'spacing': 40,
          'children': [
            {'kind': 'text', 'content': 'a'},
            {'kind': 'text', 'content': 'b'},
          ],
        }),
      );
      final gap =
          tester.getTopLeft(find.text('b')).dy -
          tester.getBottomLeft(find.text('a')).dy;
      expect(gap, closeTo(40, 0.5));
    });

    testWidgets('spacing is not applied to a single child', (tester) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'stack',
          'spacing': 40,
          'children': [
            {'kind': 'text', 'content': 'solo'},
          ],
        }),
      );
      expect(find.byType(SizedBox), findsNothing);
    });

    testWidgets('zero and negative spacing insert nothing', (tester) async {
      for (final spacing in const [0, -20]) {
        await tester.pumpWidget(
          _one({
            'kind': 'stack',
            'spacing': spacing,
            'children': const [
              {'kind': 'text', 'content': 'a'},
              {'kind': 'text', 'content': 'b'},
            ],
          }),
        );
        expect(
          tester.getTopLeft(find.text('b')).dy -
              tester.getBottomLeft(find.text('a')).dy,
          closeTo(0, 0.5),
          reason: '$spacing',
        );
      }
    });

    testWidgets('empty children render an empty stack', (tester) async {
      await tester.pumpWidget(_one(const {'kind': 'stack', 'children': []}));
      expect(tester.takeException(), isNull);
      expect(find.byType(Text), findsNothing);
    });

    testWidgets('alignItems maps onto the cross axis', (tester) async {
      for (final entry in const {
        'center': CrossAxisAlignment.center,
        'end': CrossAxisAlignment.end,
        // `stretch` on a row would demand a bounded height the renderer never
        // has, so it deliberately degrades to `start` rather than throwing.
        'stretch': CrossAxisAlignment.start,
        'start': CrossAxisAlignment.start,
        'bogus': CrossAxisAlignment.start,
      }.entries) {
        await tester.pumpWidget(
          _host(
            SizedBox(
              width: 400,
              height: 400,
              child: CrepusView.fromIr({
                'version': 5,
                'root': [
                  {
                    'kind': 'stack',
                    'axis': 'row',
                    'alignItems': entry.key,
                    'children': const [
                      {'kind': 'text', 'content': 'x'},
                    ],
                  },
                ],
              }),
            ),
          ),
        );
        final row = tester.widget<Row>(
          find
              .descendant(
                of: find.byType(CrepusView),
                matching: find.byType(Row),
              )
              .first,
        );
        expect(row.crossAxisAlignment, entry.value, reason: entry.key);
      }
    });

    testWidgets('a column stretches its children, a row never does', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'stack',
          'alignItems': 'stretch',
          'children': [
            {'kind': 'text', 'content': 'x'},
          ],
        }),
      );
      expect(
        tester
            .widget<Column>(
              find
                  .ancestor(of: find.text('x'), matching: find.byType(Column))
                  .first,
            )
            .crossAxisAlignment,
        CrossAxisAlignment.stretch,
      );
      expect(tester.takeException(), isNull);

      // The row form is the one that would otherwise blow up on unbounded
      // height; it must render cleanly.
      await tester.pumpWidget(
        _one(const {
          'kind': 'stack',
          'axis': 'row',
          'alignItems': 'stretch',
          'children': [
            {'kind': 'text', 'content': 'y'},
          ],
        }),
      );
      expect(tester.takeException(), isNull);
      expect(find.text('y'), findsOneWidget);
    });

    testWidgets('justifyContent maps onto the main axis', (tester) async {
      for (final entry in const {
        'center': MainAxisAlignment.center,
        'end': MainAxisAlignment.end,
        'between': MainAxisAlignment.spaceBetween,
        'around': MainAxisAlignment.spaceAround,
        'evenly': MainAxisAlignment.spaceEvenly,
        'bogus': MainAxisAlignment.start,
      }.entries) {
        await tester.pumpWidget(
          _one({
            'kind': 'stack',
            'justifyContent': entry.key,
            'children': const [
              {'kind': 'text', 'content': 'x'},
            ],
          }),
        );
        final column = tester.widget<Column>(
          find
              .ancestor(of: find.text('x'), matching: find.byType(Column))
              .first,
        );
        expect(column.mainAxisAlignment, entry.value, reason: entry.key);
      }
    });

    testWidgets('long press surfaces the action', (tester) async {
      final actions = <String>[];
      await tester.pumpWidget(
        _one(const {
          'kind': 'stack',
          'onLongPress': 'stack:held',
          'children': [
            {'kind': 'text', 'content': 'hold me'},
          ],
        }, onAction: actions.add),
      );
      await tester.longPress(find.text('hold me'));
      expect(actions, ['stack:held']);
    });
  });

  group('scroll', () {
    testWidgets('vertical scroll wraps children in a scroll view', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'scroll',
          'children': [
            {'kind': 'text', 'content': 'scrolled'},
          ],
        }),
      );
      final view = tester.widget<SingleChildScrollView>(
        find.byType(SingleChildScrollView),
      );
      expect(view.scrollDirection, Axis.vertical);
      expect(find.text('scrolled'), findsOneWidget);
    });

    testWidgets('row axis scrolls horizontally', (tester) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'scroll',
          'axis': 'row',
          'children': [
            {'kind': 'text', 'content': 'sideways'},
          ],
        }),
      );
      expect(
        tester
            .widget<SingleChildScrollView>(find.byType(SingleChildScrollView))
            .scrollDirection,
        Axis.horizontal,
      );
    });

    testWidgets('an empty scroll renders without throwing', (tester) async {
      await tester.pumpWidget(_one(const {'kind': 'scroll', 'children': []}));
      expect(tester.takeException(), isNull);
    });
  });

  group('button', () {
    testWidgets('renders its label', (tester) async {
      await tester.pumpWidget(_one(const {'kind': 'button', 'label': 'Go'}));
      expect(find.widgetWithText(TextButton, 'Go'), findsOneWidget);
    });

    testWidgets('a button with no onClick is disabled', (tester) async {
      await tester.pumpWidget(_one(const {'kind': 'button', 'label': 'Inert'}));
      expect(
        tester.widget<TextButton>(find.byType(TextButton)).onPressed,
        isNull,
      );
    });

    testWidgets('an empty label still renders a tappable button', (
      tester,
    ) async {
      final actions = <String>[];
      await tester.pumpWidget(
        _one(const {'kind': 'button', 'onClick': 'go'}, onAction: actions.add),
      );
      await tester.tap(find.byType(TextButton));
      expect(actions, ['go']);
    });

    testWidgets('an empty action string dispatches nothing', (tester) async {
      final actions = <String>[];
      await tester.pumpWidget(
        _one(const {
          'kind': 'button',
          'label': 'Go',
          'onClick': '',
        }, onAction: actions.add),
      );
      // An empty onClick is still non-null, so the button is enabled...
      await tester.tap(find.text('Go'));
      // ...but nothing is surfaced to the host.
      expect(actions, isEmpty);
    });

    testWidgets('taps with no onAction handler are swallowed', (tester) async {
      await tester.pumpWidget(
        _one(const {'kind': 'button', 'label': 'Go', 'onClick': 'go'}),
      );
      await tester.tap(find.text('Go'));
      expect(tester.takeException(), isNull);
    });

    testWidgets('a button carries both click and long-press actions', (
      tester,
    ) async {
      final actions = <String>[];
      await tester.pumpWidget(
        _one(const {
          'kind': 'button',
          'label': 'Go',
          'onClick': 'click',
          'onLongPress': 'hold',
        }, onAction: actions.add),
      );
      await tester.longPress(find.text('Go'));
      expect(actions, ['hold']);
    });
  });

  group('toggle and checkbox', () {
    testWidgets('toggle reflects checked and renders its label', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {'kind': 'toggle', 'label': 'Flag', 'checked': true}),
      );
      expect(find.text('Flag'), findsOneWidget);
      expect(tester.widget<Switch>(find.byType(Switch)).value, isTrue);
    });

    testWidgets('a toggle with no onChange is disabled', (tester) async {
      await tester.pumpWidget(_one(const {'kind': 'toggle', 'label': 'X'}));
      expect(tester.widget<Switch>(find.byType(Switch)).onChanged, isNull);
    });

    testWidgets('an empty toggle label renders only the control', (
      tester,
    ) async {
      await tester.pumpWidget(_one(const {'kind': 'toggle'}));
      expect(find.byType(Switch), findsOneWidget);
      expect(
        find.descendant(
          of: find.byType(CrepusView),
          matching: find.byType(Text),
        ),
        findsNothing,
      );
    });

    testWidgets('toggle long press surfaces its own action', (tester) async {
      final actions = <String>[];
      await tester.pumpWidget(
        _one(const {
          'kind': 'toggle',
          'label': 'Flag',
          'onChange': 'changed',
          'onLongPress': 'held',
        }, onAction: actions.add),
      );
      await tester.longPress(find.text('Flag'));
      expect(actions, ['held']);
    });

    testWidgets('checkbox reflects checked and dispatches onChange', (
      tester,
    ) async {
      final actions = <String>[];
      await tester.pumpWidget(
        _one(const {
          'kind': 'checkbox',
          'label': 'Done',
          'checked': true,
          'onChange': 'toggle:done',
        }, onAction: actions.add),
      );
      expect(tester.widget<Checkbox>(find.byType(Checkbox)).value, isTrue);
      await tester.tap(find.byType(Checkbox));
      expect(actions, ['toggle:done']);
    });

    testWidgets('a checkbox with no onChange is disabled', (tester) async {
      await tester.pumpWidget(_one(const {'kind': 'checkbox'}));
      expect(tester.widget<Checkbox>(find.byType(Checkbox)).onChanged, isNull);
    });
  });

  group('progress and meter', () {
    Future<double?> progressValue(
      WidgetTester tester,
      Map<String, Object?> node,
    ) async {
      await tester.pumpWidget(_one(node));
      return tester
          .widget<LinearProgressIndicator>(find.byType(LinearProgressIndicator))
          .value;
    }

    testWidgets('progress and meter fit narrow bounded containers', (
      tester,
    ) async {
      for (final kind in ['progress', 'meter']) {
        for (final width in [0.0, 10.0, 24.0, 64.0, 120.0]) {
          await tester.pumpWidget(
            _host(
              Align(
                alignment: Alignment.topLeft,
                child: SizedBox(
                  width: width,
                  child: CrepusView.fromIr({
                    'version': 5,
                    'root': [
                      {'kind': kind, 'value': 100, 'min': 0, 'max': 100},
                    ],
                  }),
                ),
              ),
            ),
          );
          expect(tester.takeException(), isNull, reason: '$kind width=$width');
          expect(find.text('100%'), findsOneWidget);
          expect(
            tester
                .widget<LinearProgressIndicator>(
                  find.byType(LinearProgressIndicator),
                )
                .value,
            1.0,
          );
        }
      }
    });

    testWidgets('progress preserves large accessible percentage text', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          MediaQuery(
            data: const MediaQueryData(textScaler: TextScaler.linear(5)),
            child: Align(
              alignment: Alignment.topLeft,
              child: SizedBox(
                width: 200,
                child: CrepusView.fromIr({
                  'version': 5,
                  'root': [
                    {'kind': 'progress', 'value': 100, 'max': 100},
                  ],
                }),
              ),
            ),
          ),
        ),
      );
      expect(tester.takeException(), isNull);
      expect(find.text('100%'), findsOneWidget);
      expect(tester.getSize(find.text('100%')).height, greaterThan(40));
    });

    testWidgets('progress maps value/max onto a 0..1 fraction', (tester) async {
      expect(
        await progressValue(tester, const {
          'kind': 'progress',
          'value': 25,
          'max': 100,
        }),
        closeTo(0.25, 1e-9),
      );
    });

    testWidgets('progress clamps out-of-range and negative values', (
      tester,
    ) async {
      expect(
        await progressValue(tester, const {
          'kind': 'progress',
          'value': 500,
          'max': 100,
        }),
        1.0,
      );
      expect(
        await progressValue(tester, const {
          'kind': 'progress',
          'value': -500,
          'max': 100,
        }),
        0.0,
      );
    });

    testWidgets('a zero or negative max degrades to zero, not NaN', (
      tester,
    ) async {
      for (final max in const [0, -10]) {
        final value = await progressValue(tester, {
          'kind': 'progress',
          'value': 50,
          'max': max,
        });
        expect(value, 0.0, reason: 'max=$max');
        expect(value!.isNaN, isFalse);
      }
    });

    testWidgets('progress renders its label only when non-empty', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {'kind': 'progress', 'label': 'Setup', 'value': 1}),
      );
      expect(find.text('Setup'), findsOneWidget);
      await tester.pumpWidget(
        _one(const {'kind': 'progress', 'label': '', 'value': 1, 'max': 100}),
      );
      expect(find.text('Setup'), findsNothing);
      expect(find.text('1%'), findsOneWidget);
    });

    testWidgets('progress and meter always show a percentage', (tester) async {
      await tester.pumpWidget(
        _one(const {'kind': 'progress', 'value': 2, 'max': 5}),
      );
      expect(find.text('40%'), findsOneWidget);
      await tester.pumpWidget(
        _one(const {'kind': 'meter', 'value': 3, 'min': 0, 'max': 10}),
      );
      expect(find.text('30%'), findsOneWidget);
    });

    testWidgets('meter maps value across min..max', (tester) async {
      expect(
        await progressValue(tester, const {
          'kind': 'meter',
          'value': 5,
          'min': 0,
          'max': 10,
        }),
        closeTo(0.5, 1e-9),
      );
      expect(
        await progressValue(tester, const {
          'kind': 'meter',
          'value': 15,
          'min': 10,
          'max': 20,
        }),
        closeTo(0.5, 1e-9),
      );
    });

    testWidgets('an inverted meter range degrades to zero', (tester) async {
      expect(
        await progressValue(tester, const {
          'kind': 'meter',
          'value': 5,
          'min': 10,
          'max': 0,
        }),
        0.0,
      );
    });

    testWidgets('an equal-bound meter fills when its value reaches the bound', (
      tester,
    ) async {
      for (final value in [4.0, 5.0, 6.0]) {
        final fraction = value >= 5 ? 1.0 : 0.0;
        expect(
          await progressValue(tester, {
            'kind': 'meter',
            'value': value,
            'min': 5,
            'max': 5,
          }),
          fraction,
        );
        expect(find.text('${(fraction * 100).round()}%'), findsOneWidget);
        expect(tester.takeException(), isNull);
      }
    });

    testWidgets('meter clamps values outside the range', (tester) async {
      expect(
        await progressValue(tester, const {
          'kind': 'meter',
          'value': -100,
          'min': 0,
          'max': 10,
        }),
        0.0,
      );
      expect(
        await progressValue(tester, const {
          'kind': 'meter',
          'value': 100,
          'min': 0,
          'max': 10,
        }),
        1.0,
      );
    });

    testWidgets('meter renders its label', (tester) async {
      await tester.pumpWidget(
        _one(const {'kind': 'meter', 'label': 'Disk', 'value': 1}),
      );
      expect(find.text('Disk'), findsOneWidget);
    });
  });

  group('badge, divider, spacer', () {
    testWidgets('each tone maps to a distinct colour', (tester) async {
      final seen = <String, Color?>{};
      for (final tone in const [
        'positive',
        'success',
        'warning',
        'negative',
        'danger',
        'info',
        'unknown-tone',
        null,
      ]) {
        await tester.pumpWidget(
          _one({'kind': 'badge', 'label': 'B', 'tone': tone}),
        );
        seen['$tone'] = tester.widget<Text>(find.text('B')).style!.color;
      }
      expect(seen['positive'], seen['success']);
      expect(seen['negative'], seen['danger']);
      expect(seen['unknown-tone'], seen['null']);
      expect(seen['positive'], isNot(seen['warning']));
      expect(seen['info'], isNot(seen['null']));
    });

    testWidgets('a badge with an empty label still renders its chip', (
      tester,
    ) async {
      await tester.pumpWidget(_one(const {'kind': 'badge'}));
      expect(find.byType(Container), findsWidgets);
      expect(tester.takeException(), isNull);
    });

    testWidgets('divider axis picks a horizontal or vertical rule', (
      tester,
    ) async {
      await tester.pumpWidget(_one(const {'kind': 'divider'}));
      expect(find.byType(Divider), findsOneWidget);
      await tester.pumpWidget(_one(const {'kind': 'divider', 'axis': 'row'}));
      expect(find.byType(VerticalDivider), findsOneWidget);
    });

    testWidgets('spacer defaults to 8 logical pixels', (tester) async {
      await tester.pumpWidget(_one(const {'kind': 'spacer'}));
      expect(tester.getSize(find.byType(SizedBox).first), const Size(8, 8));
    });

    testWidgets('a negative or zero spacer collapses instead of asserting', (
      tester,
    ) async {
      for (final size in const [0, -40]) {
        await tester.pumpWidget(_one({'kind': 'spacer', 'size': size}));
        expect(tester.takeException(), isNull, reason: '$size');
        expect(
          tester.getSize(find.byType(SizedBox).first),
          Size.zero,
          reason: '$size',
        );
      }
    });

    testWidgets('an explicit spacer size is honored', (tester) async {
      await tester.pumpWidget(_one(const {'kind': 'spacer', 'size': 24}));
      expect(tester.getSize(find.byType(SizedBox).first), const Size(24, 24));
    });
  });

  group('image', () {
    testWidgets('a non-network src renders the alt placeholder, not a fetch', (
      tester,
    ) async {
      for (final src in const [
        '',
        'file:///etc/passwd',
        'data:text/html,<script>',
        'javascript:alert(1)',
        '/local/path.png',
        'ftp://example.com/a.png',
      ]) {
        await tester.pumpWidget(
          _one({'kind': 'image', 'src': src, 'alt': 'ALT'}),
        );
        expect(find.byType(Image), findsNothing, reason: src);
        expect(find.text('ALT'), findsOneWidget, reason: src);
      }
    });

    testWidgets(
      'http/https src defaults to the non-fetch placeholder, not Image.network',
      (tester) async {
        for (final src in const [
          'http://example.com/a.png',
          'https://example.com/a.png',
        ]) {
          await tester.pumpWidget(
            _one({'kind': 'image', 'src': src, 'alt': 'ALT'}),
          );
          expect(find.byType(Image), findsNothing, reason: src);
          expect(find.text('ALT'), findsOneWidget, reason: src);
        }
      },
    );

    testWidgets(
      'allowNetworkImages: true opts in to Image.network for http/https (fromIr)',
      (tester) async {
        for (final src in const [
          'http://example.com/a.png',
          'https://example.com/a.png',
        ]) {
          await tester.pumpWidget(
            _one({
              'kind': 'image',
              'src': src,
              'alt': 'ALT',
            }, allowNetworkImages: true),
          );
          expect(find.byType(Image), findsOneWidget, reason: src);
        }
      },
    );

    testWidgets(
      'allowNetworkImages: true opts in fromSource the same as fromIr',
      (tester) async {
        await tester.pumpWidget(
          _host(
            CrepusView.fromSource(
              'image src="https://example.com/a.png" alt="ALT"',
              allowNetworkImages: true,
            ),
          ),
        );
        expect(find.byType(Image), findsOneWidget);
        expect(find.text('ALT'), findsNothing);
      },
    );

    testWidgets('fromSource defaults to non-fetch placeholder for http/https', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          CrepusView.fromSource(
            'image src="https://example.com/a.png" alt="ALT"',
          ),
        ),
      );
      expect(find.byType(Image), findsNothing);
      expect(find.text('ALT'), findsOneWidget);
    });

    testWidgets('a missing alt renders an empty placeholder', (tester) async {
      await tester.pumpWidget(_one(const {'kind': 'image', 'src': 'x'}));
      expect(find.text(''), findsOneWidget);
    });

    testWidgets('image long press surfaces its action', (tester) async {
      final actions = <String>[];
      await tester.pumpWidget(
        _one(const {
          'kind': 'image',
          'src': 'local',
          'alt': 'ALT',
          'onLongPress': 'open',
        }, onAction: actions.add),
      );
      await tester.longPress(find.text('ALT'));
      expect(actions, ['open']);
    });
  });

  group('list and listItem', () {
    testWidgets('an unordered list marks every item with a bullet', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'list',
          'children': [
            {'kind': 'text', 'content': 'a'},
            {'kind': 'text', 'content': 'b'},
          ],
        }),
      );
      expect(find.text('•'), findsNWidgets(2));
      expect(find.text('1.'), findsNothing);
    });

    testWidgets('an ordered list numbers items from one', (tester) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'list',
          'ordered': true,
          'children': [
            {'kind': 'text', 'content': 'a'},
            {'kind': 'text', 'content': 'b'},
            {'kind': 'text', 'content': 'c'},
          ],
        }),
      );
      expect(find.text('1.'), findsOneWidget);
      expect(find.text('2.'), findsOneWidget);
      expect(find.text('3.'), findsOneWidget);
    });

    testWidgets('an empty list renders nothing but does not throw', (
      tester,
    ) async {
      await tester.pumpWidget(_one(const {'kind': 'list', 'children': []}));
      expect(tester.takeException(), isNull);
      expect(find.text('•'), findsNothing);
    });

    testWidgets('listItem renders its children and long press', (tester) async {
      final actions = <String>[];
      await tester.pumpWidget(
        _one(const {
          'kind': 'list',
          'children': [
            {
              'kind': 'listItem',
              'onLongPress': 'item:held',
              'children': [
                {'kind': 'text', 'content': 'inner'},
              ],
            },
          ],
        }, onAction: actions.add),
      );
      expect(find.text('inner'), findsOneWidget);
      await tester.longPress(find.text('inner'));
      expect(actions, ['item:held']);
    });

    testWidgets('an empty listItem does not consume a numbered slot', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'list',
          'ordered': true,
          'children': [
            {'kind': 'listItem', 'children': []},
            {'kind': 'text', 'content': 'second'},
          ],
        }),
      );
      expect(find.text('1.'), findsOneWidget);
      expect(find.text('2.'), findsNothing);
      expect(find.text('second'), findsOneWidget);
    });

    testWidgets('listitem inline text renders without nested children', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          CrepusView.fromSource('''
list
  listitem "Email from Sam"
  listitem label="Invoice reminder"
'''),
        ),
      );
      expect(find.text('Email from Sam'), findsOneWidget);
      expect(find.text('Invoice reminder'), findsOneWidget);
      expect(find.text('•'), findsNWidgets(2));
    });

    testWidgets('listitem onclick dispatches through onAction', (tester) async {
      final actions = <String>[];
      await tester.pumpWidget(
        _one(const {
          'kind': 'list',
          'children': [
            {
              'kind': 'listItem',
              'onClick': 'prompt:Open the thread',
              'children': [
                {'kind': 'text', 'content': 'Tap me'},
              ],
            },
          ],
        }, onAction: actions.add),
      );
      await tester.tap(find.text('Tap me'));
      expect(actions, ['prompt:Open the thread']);
    });

    testWidgets('text bind resolves from the data scope', (tester) async {
      await tester.pumpWidget(
        _one(
          const {'kind': 'text', 'content': '', 'bind': 'subject'},
          data: const {'subject': 'Hello inbox'},
        ),
      );
      expect(find.text('Hello inbox'), findsOneWidget);
    });
  });

  group('link', () {
    testWidgets('renders its children and taps dispatch the href', (
      tester,
    ) async {
      final actions = <String>[];
      await tester.pumpWidget(
        _one(const {
          'kind': 'link',
          'href': 'https://example.com/docs',
          'children': [
            {'kind': 'text', 'content': 'Read the docs'},
            {'kind': 'badge', 'label': 'new'},
          ],
        }, onAction: actions.add),
      );
      expect(find.text('Read the docs'), findsOneWidget);
      expect(find.text('new'), findsOneWidget);
      await tester.tap(find.text('Read the docs'));
      expect(actions, ['https://example.com/docs']);
    });

    testWidgets('a link with no renderable children renders nothing', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'link',
          'href': 'https://example.com',
          'children': [],
        }),
      );
      expect(find.byType(GestureDetector), findsNothing);
    });

    testWidgets('a link with an empty href is inert but still renders', (
      tester,
    ) async {
      final actions = <String>[];
      await tester.pumpWidget(
        _one(const {
          'kind': 'link',
          'children': [
            {'kind': 'text', 'content': 'Nowhere'},
          ],
        }, onAction: actions.add),
      );
      expect(find.text('Nowhere'), findsOneWidget);
      await tester.tap(find.text('Nowhere'));
      expect(actions, isEmpty);
    });

    testWidgets('link style is applied like any other container', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'link',
          'href': 'https://example.com',
          'style': {'padding': 6},
          'children': [
            {'kind': 'text', 'content': 'Styled link'},
          ],
        }),
      );
      final padding = tester.widget<Padding>(
        find
            .ancestor(
              of: find.text('Styled link'),
              matching: find.byType(Padding),
            )
            .last,
      );
      expect(padding.padding, const EdgeInsets.all(6));
    });
  });

  group('if', () {
    Future<void> pumpIf(
      WidgetTester tester,
      String condition,
      Map<String, Object?> data, {
      bool withElse = true,
    }) => tester.pumpWidget(
      _one({
        'kind': 'if',
        'condition': condition,
        'thenChildren': const [
          {'kind': 'text', 'content': 'THEN'},
        ],
        if (withElse)
          'elseChildren': const [
            {'kind': 'text', 'content': 'ELSE'},
          ],
      }, data: data),
    );

    testWidgets('a bare variable is evaluated for truthiness', (tester) async {
      const truthy = <Object?>[
        true,
        1,
        -1,
        'x',
        [1],
        {'a': 1},
      ];
      for (final value in truthy) {
        await pumpIf(tester, 'flag', {'flag': value});
        expect(find.text('THEN'), findsOneWidget, reason: '$value');
      }
      const falsy = <Object?>[false, 0, '', <Object?>[], <String, Object?>{}];
      for (final value in falsy) {
        await pumpIf(tester, 'flag', {'flag': value});
        expect(find.text('ELSE'), findsOneWidget, reason: '$value');
      }
    });

    testWidgets('a bare literal is resolved the same way as a comparison '
        'operand', (tester) async {
      await pumpIf(tester, 'true', const {});
      expect(find.text('THEN'), findsOneWidget);
      await pumpIf(tester, 'false', const {});
      expect(find.text('ELSE'), findsOneWidget);
      await pumpIf(tester, '1', const {});
      expect(find.text('THEN'), findsOneWidget);
      await pumpIf(tester, '0', const {});
      expect(find.text('ELSE'), findsOneWidget);
      await pumpIf(tester, '"lit"', const {});
      expect(find.text('THEN'), findsOneWidget);
      await pumpIf(tester, '""', const {});
      expect(find.text('ELSE'), findsOneWidget);
      await pumpIf(tester, '!true', const {});
      expect(find.text('ELSE'), findsOneWidget);
      await pumpIf(tester, 'x == true', const {'x': true});
      expect(find.text('THEN'), findsOneWidget);
    });

    testWidgets('a missing binding is falsy', (tester) async {
      await pumpIf(tester, 'nope', const {});
      expect(find.text('ELSE'), findsOneWidget);
    });

    testWidgets('negation inverts truthiness', (tester) async {
      await pumpIf(tester, '!flag', const {'flag': false});
      expect(find.text('THEN'), findsOneWidget);
      await pumpIf(tester, '!flag', const {'flag': true});
      expect(find.text('ELSE'), findsOneWidget);
    });

    testWidgets('equality compares against literals and other bindings', (
      tester,
    ) async {
      await pumpIf(tester, 'status == "done"', const {'status': 'done'});
      expect(find.text('THEN'), findsOneWidget);
      await pumpIf(tester, 'status == "done"', const {'status': 'open'});
      expect(find.text('ELSE'), findsOneWidget);
      await pumpIf(tester, 'status != "done"', const {'status': 'open'});
      expect(find.text('THEN'), findsOneWidget);
      await pumpIf(tester, 'a == b', const {'a': 1, 'b': 1});
      expect(find.text('THEN'), findsOneWidget);
      await pumpIf(tester, 'count == 3', const {'count': 3});
      expect(find.text('THEN'), findsOneWidget);
      await pumpIf(tester, 'flag == true', const {'flag': true});
      expect(find.text('THEN'), findsOneWidget);
      await pumpIf(tester, 'flag == false', const {'flag': false});
      expect(find.text('THEN'), findsOneWidget);
    });

    testWidgets('operators inside quoted operands stay literal', (
      tester,
    ) async {
      for (final condition in [
        '"a==b" != "other"',
        '"a!=b" == "a!=b"',
        '"=="',
        'value != "a==b"',
      ]) {
        await pumpIf(tester, condition, const {'value': 'different'});
        expect(find.text('THEN'), findsOneWidget, reason: condition);
        expect(tester.takeException(), isNull);
      }
      await pumpIf(tester, '"a==b" == "other"', const {});
      expect(find.text('ELSE'), findsOneWidget);
    });

    testWidgets('quoted operands decode the same escapes as template strings', (
      tester,
    ) async {
      for (final entry in <String, String>{
        r'value == "a\"b"': 'a"b',
        r'value == "line\nnext"': 'line\nnext',
        r'value == "C:\\tools"': r'C:\tools',
        r'value == "unknown\q"': r'unknown\q',
      }.entries) {
        await pumpIf(tester, entry.key, {'value': entry.value});
        expect(find.text('THEN'), findsOneWidget, reason: entry.key);
        expect(tester.takeException(), isNull);
      }
    });

    testWidgets(
      'malformed quoted comparisons and repeated operators are falsy',
      (tester) async {
        for (final condition in [
          'value != "unterminated',
          '"a"tail != "b"',
          '1 == 1 == 1',
          '!"unterminated',
          '!!x',
        ]) {
          await pumpIf(tester, condition, const {'value': 'different'});
          expect(find.text('ELSE'), findsOneWidget, reason: condition);
          expect(tester.takeException(), isNull);
        }
      },
    );

    testWidgets('nonfinite numeric conditions cannot select a branch', (
      tester,
    ) async {
      for (final condition in [
        'NaN',
        'Infinity',
        '!NaN',
        'NaN == NaN',
        'value',
        'value == value',
        'value != 0',
      ]) {
        await pumpIf(tester, condition, const {'value': double.nan});
        expect(find.text('ELSE'), findsOneWidget, reason: condition);
        expect(tester.takeException(), isNull);
      }
      await pumpIf(tester, '"NaN"', const {});
      expect(find.text('THEN'), findsOneWidget);
    });

    testWidgets('numeric equality compares values across int and double', (
      tester,
    ) async {
      for (final condition in [
        'count == 1.0',
        '1 == decimal',
        'count == decimal',
      ]) {
        await pumpIf(tester, condition, const {'count': 1, 'decimal': 1.0});
        expect(find.text('THEN'), findsOneWidget, reason: condition);
      }
      await pumpIf(tester, 'count != decimal', const {
        'count': 1,
        'decimal': 1.0,
      });
      expect(find.text('ELSE'), findsOneWidget);
      await pumpIf(tester, 'count == decimal', const {
        'count': 1,
        'decimal': 1.5,
      });
      expect(find.text('ELSE'), findsOneWidget);
    });

    testWidgets('null remains a literal when scope contains a null key', (
      tester,
    ) async {
      final data = <String, Object?>{'value': null, 'null': 'overridden'};
      await pumpIf(tester, 'value == null', data);
      expect(find.text('THEN'), findsOneWidget);
      await pumpIf(tester, 'null', data);
      expect(find.text('ELSE'), findsOneWidget);
      await pumpIf(tester, 'value != null', data);
      expect(find.text('ELSE'), findsOneWidget);
      await pumpIf(tester, '"null"', data);
      expect(find.text('THEN'), findsOneWidget);
    });

    testWidgets('a dotted path works on both sides of a comparison', (
      tester,
    ) async {
      await pumpIf(tester, 'user.role == "admin"', const {
        'user': {'role': 'admin'},
      });
      expect(find.text('THEN'), findsOneWidget);
    });

    testWidgets('unsupported operands never compare as two missing values', (
      tester,
    ) async {
      for (final condition in [
        'a && b == missing',
        'missing == a || b',
        'call() == missing',
        '1 === 1',
        'a >= b == missing',
        'a..b == missing',
        'a[0] == missing',
        'a + b == missing',
      ]) {
        await pumpIf(tester, condition, const {});
        expect(find.text('ELSE'), findsOneWidget, reason: condition);
        expect(find.text('THEN'), findsNothing, reason: condition);
      }
    });

    testWidgets('an empty or nonsensical condition is falsy, never thrown', (
      tester,
    ) async {
      for (final condition in const [
        '',
        '   ',
        '==',
        '!',
        '!!x',
        'a ==',
        '== b',
        'a && b',
        'process.exit(1)',
        r'${x}',
        '1 == 1 == 1',
      ]) {
        await pumpIf(tester, condition, const {});
        expect(tester.takeException(), isNull, reason: condition);
      }
    });

    testWidgets('a false condition with no else branch renders nothing', (
      tester,
    ) async {
      await pumpIf(tester, 'flag', const {}, withElse: false);
      expect(find.text('THEN'), findsNothing);
      expect(find.text('ELSE'), findsNothing);
    });

    testWidgets('a branch whose children all render nothing is omitted', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(
          const {
            'kind': 'if',
            'condition': 'flag',
            'thenChildren': [
              {'kind': 'forEach', 'bind': 'missing', 'itemBody': []},
            ],
          },
          data: const {'flag': true},
        ),
      );
      expect(tester.takeException(), isNull);
    });
  });

  group('forEach', () {
    testWidgets(
      'runtime expansion respects the node budget and resets each build',
      (tester) async {
        for (var build = 0; build < 2; build++) {
          await tester.pumpWidget(
            _host(
              CrepusView.fromIr(
                {
                  'version': 5,
                  'root': [
                    {
                      'kind': 'forEach',
                      'bind': 'items',
                      'itemName': 'row',
                      'itemBody': [
                        {'kind': 'text', 'content': 'row={row}'},
                      ],
                    },
                  ],
                },
                key: ValueKey(build),
                limits: const CrepusLimits(maxNodes: 5),
                data: {'items': List.generate(10000, (i) => i)},
              ),
            ),
          );
          expect(find.text('row=0'), findsOneWidget);
          expect(find.text('row=3'), findsOneWidget);
          expect(find.text('row=4'), findsNothing);
          expect(tester.takeException(), isNull);
        }
      },
    );

    testWidgets('resolves nested list bindings and preserves exact keys', (
      tester,
    ) async {
      for (final scenario in [
        (false, 'data.items'),
        (true, 'data.items'),
        (false, ' data.items '),
        (true, ' data.items '),
      ]) {
        final (exactKey, binding) = scenario;
        await tester.pumpWidget(
          _one(
            {
              'kind': 'forEach',
              'bind': binding,
              'itemName': 'row',
              'itemBody': [
                {'kind': 'text', 'content': 'row={row.name}'},
              ],
            },
            data: {
              'data': {
                'items': [
                  {'name': 'nested'},
                ],
              },
              if (exactKey)
                'data.items': [
                  {'name': 'exact'},
                ],
            },
          ),
        );
        expect(
          find.text(exactKey ? 'row=exact' : 'row=nested'),
          findsOneWidget,
        );
        expect(find.text(exactKey ? 'row=nested' : 'row=exact'), findsNothing);
        expect(tester.takeException(), isNull);
      }
    });

    testWidgets('expands the item body once per element', (tester) async {
      await tester.pumpWidget(
        _one(
          const {
            'kind': 'forEach',
            'bind': 'items',
            'itemName': 'row',
            'itemBody': [
              {'kind': 'text', 'content': 'row={row}'},
            ],
          },
          data: const {
            'items': ['a', 'b', 'c'],
          },
        ),
      );
      expect(find.text('row=a'), findsOneWidget);
      expect(find.text('row=b'), findsOneWidget);
      expect(find.text('row=c'), findsOneWidget);
    });

    testWidgets('item scope shadows but does not destroy the outer scope', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(
          const {
            'kind': 'forEach',
            'bind': 'items',
            'itemName': 'name',
            'itemBody': [
              {'kind': 'text', 'content': '{name}@{host}'},
            ],
          },
          data: const {
            'items': ['a'],
            'name': 'outer',
            'host': 'H',
          },
        ),
      );
      expect(find.text('a@H'), findsOneWidget);
    });

    testWidgets('maps in the list resolve through dotted item paths', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(
          const {
            'kind': 'forEach',
            'bind': 'rows',
            'itemName': 'r',
            'itemBody': [
              {'kind': 'text', 'content': '{r.title}'},
            ],
          },
          data: const {
            'rows': [
              {'title': 'One'},
              {'title': 'Two'},
            ],
          },
        ),
      );
      expect(find.text('One'), findsOneWidget);
      expect(find.text('Two'), findsOneWidget);
    });

    testWidgets('a non-list, missing, or empty binding renders nothing', (
      tester,
    ) async {
      for (final data in const <Map<String, Object?>>[
        {},
        {'items': 'not a list'},
        {'items': 42},
        {
          'items': {'a': 1},
        },
        {'items': null},
        {'items': []},
      ]) {
        await tester.pumpWidget(
          _one(const {
            'kind': 'forEach',
            'bind': 'items',
            'itemBody': [
              {'kind': 'text', 'content': 'ITEM'},
            ],
          }, data: data),
        );
        expect(find.text('ITEM'), findsNothing, reason: '$data');
        expect(tester.takeException(), isNull, reason: '$data');
      }
    });

    testWidgets('an empty item body renders nothing', (tester) async {
      await tester.pumpWidget(
        _one(
          const {'kind': 'forEach', 'bind': 'items', 'itemBody': []},
          data: const {
            'items': [1, 2],
          },
        ),
      );
      expect(tester.takeException(), isNull);
    });

    testWidgets('a nested forEach expands the cartesian product', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(
          const {
            'kind': 'forEach',
            'bind': 'outer',
            'itemName': 'o',
            'itemBody': [
              {
                'kind': 'forEach',
                'bind': 'inner',
                'itemName': 'i',
                'itemBody': [
                  {'kind': 'text', 'content': '{o}{i}'},
                ],
              },
            ],
          },
          data: const {
            'outer': ['a', 'b'],
            'inner': [1, 2],
          },
        ),
      );
      for (final label in const ['a1', 'a2', 'b1', 'b2']) {
        expect(find.text(label), findsOneWidget, reason: label);
      }
    });
  });

  group('unsupported nodes', () {
    testWidgets('a disallowed kind renders only an inert debug marker', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'webView',
          'src': 'https://evil.example',
          'onClick': 'exfiltrate',
        }),
      );
      expect(find.text('unsupported: webView'), findsOneWidget);
      expect(find.byType(TextButton), findsNothing);
      expect(find.byType(Image), findsNothing);
    });

    testWidgets('unsupported siblings do not suppress supported ones', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          CrepusView.fromIr(const {
            'version': 5,
            'root': [
              {'kind': 'iframe'},
              {'kind': 'text', 'content': 'safe'},
              {'kind': 'dropzone'},
            ],
          }),
        ),
      );
      expect(find.text('safe'), findsOneWidget);
    });
  });

  group('style application', () {
    testWidgets('padding shorthands cascade specific over general', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'text',
          'content': 'P',
          'style': {'padding': 4, 'paddingHorizontal': 8, 'paddingTop': 16},
        }),
      );
      final padding = tester.widget<Padding>(
        find.ancestor(of: find.text('P'), matching: find.byType(Padding)).first,
      );
      expect(
        padding.padding,
        const EdgeInsets.only(top: 16, bottom: 4, left: 8, right: 8),
      );
    });

    testWidgets('no padding properties means no Padding wrapper', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'text',
          'content': 'P',
          'style': {'opacity': 0.5},
        }),
      );
      expect(
        find.descendant(
          of: find.byType(CrepusView),
          matching: find.byType(Padding),
        ),
        findsNothing,
      );
    });

    testWidgets('opacity is clamped into 0..1', (tester) async {
      const cases = <(Object, double)>[(0.5, 0.5), (-3, 0.0), (9, 1.0)];
      for (final entry in cases.map((c) => MapEntry(c.$1, c.$2))) {
        await tester.pumpWidget(
          _one({
            'kind': 'text',
            'content': 'O',
            'style': {'opacity': entry.key},
          }),
        );
        expect(
          tester
              .widget<Opacity>(
                find
                    .ancestor(
                      of: find.text('O'),
                      matching: find.byType(Opacity),
                    )
                    .first,
              )
              .opacity,
          entry.value,
          reason: '${entry.key}',
        );
      }
    });

    testWidgets('background, radius and border become a decoration', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'text',
          'content': 'D',
          'style': {
            'backgroundColor': '#112233',
            'cornerRadius': 12,
            'borderColor': '#445566',
            'borderWidth': 3,
          },
        }),
      );
      final decoration =
          tester
                  .widget<DecoratedBox>(
                    find
                        .ancestor(
                          of: find.text('D'),
                          matching: find.byType(DecoratedBox),
                        )
                        .first,
                  )
                  .decoration
              as BoxDecoration;
      expect(decoration.color, const Color(0xff112233));
      expect(decoration.borderRadius, BorderRadius.circular(12));
      expect(decoration.border!.top.color, const Color(0xff445566));
      expect(decoration.border!.top.width, 3);
    });

    testWidgets('an unparseable border colour does not crash the render', (
      tester,
    ) async {
      for (final colour in const [
        'notacolor',
        '#12',
        '#zzzzzz',
        '',
        '#1234567890',
      ]) {
        await tester.pumpWidget(
          _one({
            'kind': 'text',
            'content': 'B',
            'style': {'borderColor': colour},
          }),
        );
        expect(tester.takeException(), isNull, reason: colour);
        expect(find.text('B'), findsOneWidget, reason: colour);
      }
    });

    testWidgets('a negative corner radius or border width is ignored', (
      tester,
    ) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'text',
          'content': 'N',
          'style': {
            'cornerRadius': -8,
            'borderColor': '#ffffff',
            'borderWidth': -4,
          },
        }),
      );
      expect(tester.takeException(), isNull);
      final decoration =
          tester
                  .widget<DecoratedBox>(
                    find
                        .ancestor(
                          of: find.text('N'),
                          matching: find.byType(DecoratedBox),
                        )
                        .first,
                  )
                  .decoration
              as BoxDecoration;
      expect(decoration.borderRadius, isNull);
      expect(decoration.border!.top.width, 1);
    });

    testWidgets('colours accept 6- and 8-digit hex, with or without a hash', (
      tester,
    ) async {
      const cases = {
        '#ff0000': Color(0xffff0000),
        'ff0000': Color(0xffff0000),
        '#00ff0080': Color(0x8000ff00),
        '  #0000ff  ': Color(0xff0000ff),
      };
      for (final entry in cases.entries) {
        await tester.pumpWidget(
          _one({
            'kind': 'text',
            'content': 'C',
            'style': {'foregroundColor': entry.key},
          }),
        );
        expect(
          tester.widget<Text>(find.text('C')).style!.color,
          entry.value,
          reason: entry.key,
        );
      }
    });

    testWidgets('font weights bucket into the nearest supported weight', (
      tester,
    ) async {
      const cases = {
        100: FontWeight.w100,
        200: FontWeight.w200,
        300: FontWeight.w300,
        450: FontWeight.w400,
        550: FontWeight.w500,
        650: FontWeight.w600,
        750: FontWeight.w700,
        900: FontWeight.w900,
      };
      for (final entry in cases.entries) {
        await tester.pumpWidget(
          _one({
            'kind': 'text',
            'content': 'W',
            'style': {'fontWeight': entry.key},
          }),
        );
        expect(
          tester.widget<Text>(find.text('W')).style!.fontWeight,
          entry.value,
          reason: '${entry.key}',
        );
      }
    });

    testWidgets('text alignment aliases map onto TextAlign', (tester) async {
      const cases = {
        'center': TextAlign.center,
        'right': TextAlign.right,
        'trailing': TextAlign.right,
        'left': TextAlign.left,
        'leading': TextAlign.left,
      };
      for (final entry in cases.entries) {
        await tester.pumpWidget(
          _one({
            'kind': 'text',
            'content': 'A',
            'style': {'textAlign': entry.key},
          }),
        );
        expect(
          tester.widget<Text>(find.text('A')).textAlign,
          entry.value,
          reason: entry.key,
        );
      }
      await tester.pumpWidget(
        _one(const {
          'kind': 'text',
          'content': 'A',
          'style': {'textAlign': 'justify-ish'},
        }),
      );
      expect(tester.widget<Text>(find.text('A')).textAlign, isNull);
    });

    testWidgets('style applies to container kinds too', (tester) async {
      await tester.pumpWidget(
        _one(const {
          'kind': 'stack',
          'style': {'padding': 20},
          'children': [
            {'kind': 'text', 'content': 'inside'},
          ],
        }),
      );
      expect(tester.getTopLeft(find.text('inside')).dx, closeTo(20, 0.5));
    });
  });

  group('theming', () {
    testWidgets('an explicit CrepusTheme overrides the ambient defaults', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          CrepusView.fromIr(
            const {
              'version': 5,
              'root': [
                {'kind': 'text', 'content': 'T'},
              ],
            },
            theme: const CrepusTheme(
              textColor: Color(0xff00ff00),
              baseFontSize: 30,
            ),
          ),
        ),
      );
      final style = tester.widget<Text>(find.text('T')).style!;
      expect(style.color, const Color(0xff00ff00));
      expect(style.fontSize, 30);
    });

    testWidgets('defaults are derived from the ambient ThemeData', (
      tester,
    ) async {
      final scheme = ColorScheme.fromSeed(seedColor: const Color(0xff123456));
      await tester.pumpWidget(
        MaterialApp(
          theme: ThemeData(colorScheme: scheme),
          home: Scaffold(
            body: CrepusView.fromIr(const {
              'version': 5,
              'root': [
                {'kind': 'text', 'content': 'T'},
              ],
            }),
          ),
        ),
      );
      expect(
        tester.widget<Text>(find.text('T')).style!.color,
        scheme.onSurface,
      );
    });

    testWidgets('a style colour still beats the theme colour', (tester) async {
      await tester.pumpWidget(
        _host(
          CrepusView.fromIr(const {
            'version': 5,
            'root': [
              {
                'kind': 'text',
                'content': 'T',
                'style': {'foregroundColor': '#abcdef'},
              },
            ],
          }, theme: const CrepusTheme(textColor: Color(0xff00ff00))),
        ),
      );
      expect(
        tester.widget<Text>(find.text('T')).style!.color,
        const Color(0xffabcdef),
      );
    });
  });

  group('document-level rendering', () {
    testWidgets('an empty document renders an empty column', (tester) async {
      await tester.pumpWidget(
        _host(CrepusView.fromIr(const {'version': 5, 'root': []})),
      );
      expect(tester.takeException(), isNull);
      expect(
        find.descendant(
          of: find.byType(CrepusView),
          matching: find.byType(Text),
        ),
        findsNothing,
      );
    });

    testWidgets('an over-limit document renders nothing at all', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          CrepusView.fromIr({
            'version': 5,
            'root': List.generate(
              200,
              (i) => {'kind': 'text', 'content': 'n$i'},
            ),
          }),
        ),
      );
      expect(find.text('n0'), findsNothing);
    });

    testWidgets('the decoded document is exposed for host introspection', (
      tester,
    ) async {
      final view = CrepusView.fromIr(const {
        'version': 5,
        'root': [
          {'kind': 'text', 'content': 'x'},
        ],
      });
      expect(view.ir.version, 5);
      expect(view.ir.root.single, isA<TextNode>());
    });

    testWidgets('containers and leaves nest to the depth cap', (tester) async {
      Map<String, Object?> nest(int n) => n == 0
          ? {'kind': 'text', 'content': 'bottom'}
          : {
              'kind': 'stack',
              'spacing': 2,
              'children': [nest(n - 1)],
            };
      await tester.pumpWidget(
        _host(
          CrepusView.fromIr({
            'version': 5,
            'root': [nest(7)],
          }),
        ),
      );
      expect(find.text('bottom'), findsOneWidget);
    });

    testWidgets('a document mixing every kind renders in one pass', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          CrepusView.fromIr(
            const {
              'version': 5,
              'root': [
                {'kind': 'text', 'content': 'T'},
                {'kind': 'badge', 'label': 'B'},
                {'kind': 'button', 'label': 'Btn', 'onClick': 'go'},
                {'kind': 'toggle', 'label': 'Tg', 'onChange': 'c'},
                {'kind': 'checkbox', 'label': 'Cb', 'onChange': 'c'},
                {'kind': 'progress', 'value': 1, 'max': 2},
                {'kind': 'meter', 'value': 1, 'max': 2},
                {'kind': 'divider'},
                {'kind': 'spacer', 'size': 4},
                {'kind': 'image', 'src': 'x', 'alt': 'IMG'},
                {
                  'kind': 'scroll',
                  'children': [
                    {'kind': 'text', 'content': 'S'},
                  ],
                },
                {
                  'kind': 'list',
                  'children': [
                    {
                      'kind': 'listItem',
                      'children': [
                        {'kind': 'text', 'content': 'LI'},
                      ],
                    },
                  ],
                },
                {
                  'kind': 'if',
                  'condition': 'shown',
                  'thenChildren': [
                    {'kind': 'text', 'content': 'IF'},
                  ],
                },
                {
                  'kind': 'forEach',
                  'bind': 'xs',
                  'itemBody': [
                    {'kind': 'text', 'content': 'FE'},
                  ],
                },
              ],
            },
            data: const {
              'shown': true,
              'xs': [1],
            },
          ),
        ),
      );
      expect(tester.takeException(), isNull);
      for (final label in const ['T', 'B', 'Btn', 'Tg', 'Cb', 'IMG', 'S']) {
        expect(find.text(label), findsOneWidget, reason: label);
      }
      expect(find.text('LI'), findsOneWidget);
      expect(find.text('IF'), findsOneWidget);
      expect(find.text('FE'), findsOneWidget);
      expect(find.byType(LinearProgressIndicator), findsNWidgets(2));
      expect(find.byType(Divider), findsOneWidget);
    });
  });
}
