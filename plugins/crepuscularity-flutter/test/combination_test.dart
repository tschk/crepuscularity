import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// Combination coverage. Every line in the renderer is already exercised; what
/// is not is the *interaction* between properties — style layers stacked on one
/// node, containers with contradictory alignment nested inside each other,
/// control flow nested inside control flow inside a styled container, and
/// per-node style competing with a host theme. These tests also assert the
/// resolved values (insets, colours, fractions, alignments) rather than merely
/// that a widget of some type exists.
Widget _host(Widget child, {ThemeData? theme}) => MaterialApp(
  theme: theme,
  home: Scaffold(body: child),
);

Widget _doc(
  List<Map<String, Object?>> root, {
  CrepusActionCallback? onAction,
  Map<String, Object?> data = const {},
  CrepusTheme crepusTheme = const CrepusTheme(),
  ThemeData? theme,
}) => _host(
  CrepusView.fromIr(
    {'version': 5, 'root': root},
    onAction: onAction,
    data: data,
    theme: crepusTheme,
  ),
  theme: theme,
);

Finder _under(Type type) =>
    find.descendant(of: find.byType(CrepusView), matching: find.byType(type));

/// The Row/Column a given text sits inside, skipping Flutter's own flexes by
/// taking the closest ancestor that the renderer itself created.
Finder _flexAround(String text, Type flexType) =>
    find.ancestor(of: find.text(text), matching: find.byType(flexType));

void main() {
  group('style layers stack in a fixed order with resolved values', () {
    testWidgets('padding + decoration + border + radius + opacity together', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'text',
            'content': 'T',
            'style': {
              'padding': 4,
              'paddingLeft': 11,
              'paddingTop': 3,
              'backgroundColor': '#112233',
              'borderColor': '#445566',
              'borderWidth': 2.5,
              'cornerRadius': 7,
              'opacity': 0.25,
              'fontSize': 19,
              'fontWeight': 640,
              'foregroundColor': '#aabbcc',
              'italic': true,
              'underline': true,
              'strikethrough': true,
              'textAlign': 'trailing',
            },
          },
        ]),
      );

      // Outermost → innermost: Opacity > DecoratedBox > Padding > Text.
      expect(
        find.descendant(
          of: _under(Opacity),
          matching: find.byType(DecoratedBox),
        ),
        findsOneWidget,
      );
      expect(
        find.descendant(
          of: _under(DecoratedBox),
          matching: find.byType(Padding),
        ),
        findsWidgets,
      );

      expect(tester.widget<Opacity>(_under(Opacity)).opacity, 0.25);

      final decoration =
          tester.widget<DecoratedBox>(_under(DecoratedBox)).decoration
              as BoxDecoration;
      expect(decoration.color, const Color(0xff112233));
      expect(decoration.borderRadius, BorderRadius.circular(7));
      expect(
        decoration.border,
        Border.all(color: const Color(0xff445566), width: 2.5),
      );

      final padding = tester.widget<Padding>(
        find.ancestor(of: find.text('T'), matching: find.byType(Padding)).first,
      );
      expect(
        padding.padding,
        const EdgeInsets.only(top: 3, bottom: 4, left: 11, right: 4),
      );

      final text = tester.widget<Text>(find.text('T'));
      expect(text.style!.fontSize, 19);
      expect(text.style!.fontWeight, FontWeight.w600);
      expect(text.style!.color, const Color(0xffaabbcc));
      expect(text.style!.fontStyle, FontStyle.italic);
      expect(
        text.style!.decoration,
        TextDecoration.combine(const [
          TextDecoration.underline,
          TextDecoration.lineThrough,
        ]),
      );
      expect(text.textAlign, TextAlign.right);
    });

    testWidgets('a background with no radius and no border still decorates', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'text',
            'content': 'T',
            'style': {'backgroundColor': '#ff0000'},
          },
        ]),
      );
      final decoration =
          tester.widget<DecoratedBox>(_under(DecoratedBox)).decoration
              as BoxDecoration;
      expect(decoration.color, const Color(0xffff0000));
      expect(decoration.borderRadius, isNull);
      expect(decoration.border, isNull);
      expect(_under(Opacity), findsNothing);
    });

    testWidgets('a border with an unparseable colour drops only the border', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'text',
            'content': 'T',
            'style': {
              'borderColor': 'not-a-colour',
              'borderWidth': 3,
              'backgroundColor': '#00ff00',
              'cornerRadius': 5,
            },
          },
        ]),
      );
      final decoration =
          tester.widget<DecoratedBox>(_under(DecoratedBox)).decoration
              as BoxDecoration;
      expect(decoration.border, isNull);
      expect(decoration.color, const Color(0xff00ff00));
      expect(decoration.borderRadius, BorderRadius.circular(5));
    });

    testWidgets('a negative padding is clamped to zero, never asserted', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'text',
            'content': 'T',
            'style': {
              'paddingLeft': -50,
              'paddingTop': -0.001,
              'paddingRight': 6,
              'paddingBottom': 0,
            },
          },
        ]),
      );
      expect(tester.takeException(), isNull);
      final padding = tester.widget<Padding>(
        find.ancestor(of: find.text('T'), matching: find.byType(Padding)).first,
      );
      expect(padding.padding, const EdgeInsets.only(right: 6));
    });

    testWidgets('an absurd finite magnitude is clamped to a sane extent', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'spacer',
            'size': 1e9,
            'style': {'padding': 1e9, 'cornerRadius': 1e9, 'borderWidth': 1e9},
          },
          {
            'kind': 'text',
            'content': 'T',
            'style': {'fontSize': 1e9},
          },
        ]),
      );
      // 4000pt still overflows an 800x600 test viewport — that is the host's
      // problem to scroll, not a renderer defect. What matters is that the
      // magnitude is bounded at all instead of being 1e9.
      final overflow = tester.takeException();
      expect(overflow, isA<FlutterError>());
      expect('$overflow', contains('overflowed'));
      final spacer = tester.widget<SizedBox>(_under(SizedBox).first);
      expect(spacer.width, 4000);
      expect(spacer.height, 4000);
      final padding = tester.widget<Padding>(_under(Padding).first);
      expect(padding.padding, const EdgeInsets.all(4000));
      expect(tester.widget<Text>(find.text('T')).style!.fontSize, 400);
    });

    testWidgets('an out-of-range opacity is clamped at both ends', (
      tester,
    ) async {
      for (final pair in const [
        (-5.0, 0.0),
        (0.0, 0.0),
        (0.5, 0.5),
        (1.0, 1.0),
        (99.0, 1.0),
      ]) {
        await tester.pumpWidget(
          _doc([
            {
              'kind': 'text',
              'content': 'T',
              'style': {'opacity': pair.$1},
            },
          ]),
        );
        expect(
          tester.widget<Opacity>(_under(Opacity)).opacity,
          pair.$2,
          reason: '${pair.$1}',
        );
      }
    });

    testWidgets('style on a container wraps the whole subtree once', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'stack',
            'style': {'padding': 8, 'opacity': 0.5},
            'children': [
              {
                'kind': 'text',
                'content': 'inner',
                'style': {'padding': 2, 'opacity': 0.5},
              },
            ],
          },
        ]),
      );
      // Two Opacity layers, and their effect composes rather than one winning.
      final opacities = tester
          .widgetList<Opacity>(_under(Opacity))
          .map((o) => o.opacity)
          .toList();
      expect(opacities, [0.5, 0.5]);
    });
  });

  group('nested containers with conflicting alignment', () {
    testWidgets('a row inside a column keeps each node its own alignment', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'stack',
            'axis': 'column',
            'alignItems': 'end',
            'justifyContent': 'between',
            'children': [
              {
                'kind': 'stack',
                'axis': 'row',
                'alignItems': 'center',
                'justifyContent': 'around',
                'children': [
                  {'kind': 'text', 'content': 'leaf'},
                ],
              },
            ],
          },
        ]),
      );
      final row = tester.widget<Row>(_flexAround('leaf', Row).first);
      expect(row.crossAxisAlignment, CrossAxisAlignment.center);
      expect(row.mainAxisAlignment, MainAxisAlignment.spaceAround);
      expect(row.mainAxisSize, MainAxisSize.min);

      final column = tester.widget<Column>(_flexAround('leaf', Column).first);
      expect(column.crossAxisAlignment, CrossAxisAlignment.end);
      expect(column.mainAxisAlignment, MainAxisAlignment.spaceBetween);
    });

    testWidgets('stretch is honoured on a column but neutralised on a row', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'stack',
            'axis': 'column',
            'alignItems': 'stretch',
            'children': [
              {
                'kind': 'stack',
                'axis': 'row',
                'alignItems': 'stretch',
                'children': [
                  {'kind': 'text', 'content': 'leaf'},
                ],
              },
            ],
          },
        ]),
      );
      expect(tester.takeException(), isNull);
      expect(
        tester.widget<Row>(_flexAround('leaf', Row).first).crossAxisAlignment,
        CrossAxisAlignment.start,
      );
      expect(
        tester
            .widget<Column>(_flexAround('leaf', Column).first)
            .crossAxisAlignment,
        CrossAxisAlignment.stretch,
      );
    });

    testWidgets('an unrecognised alignment falls back rather than throwing', (
      tester,
    ) async {
      for (final align in const ['baseline', 'CENTER', '', 'space-evenly']) {
        await tester.pumpWidget(
          _doc([
            {
              'kind': 'stack',
              'axis': 'row',
              'alignItems': align,
              'justifyContent': align,
              'children': const [
                {'kind': 'text', 'content': 'leaf'},
              ],
            },
          ]),
        );
        final row = tester.widget<Row>(_flexAround('leaf', Row).first);
        expect(row.crossAxisAlignment, CrossAxisAlignment.start, reason: align);
        expect(row.mainAxisAlignment, MainAxisAlignment.start, reason: align);
      }
    });

    testWidgets('spacing inserts n-1 gaps on the correct axis', (tester) async {
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'stack',
            'axis': 'row',
            'spacing': 9,
            'children': [
              {'kind': 'text', 'content': 'a'},
              {'kind': 'text', 'content': 'b'},
              {'kind': 'text', 'content': 'c'},
            ],
          },
        ]),
      );
      final row = tester.widget<Row>(_flexAround('a', Row).first);
      final gaps = row.children.whereType<SizedBox>().toList();
      expect(gaps, hasLength(2));
      expect(gaps.every((g) => g.width == 9 && g.height == null), isTrue);

      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'stack',
            'axis': 'column',
            'spacing': 9,
            'children': [
              {'kind': 'text', 'content': 'a'},
              {'kind': 'text', 'content': 'b'},
            ],
          },
        ]),
      );
      final column = tester.widget<Column>(_flexAround('a', Column).first);
      final colGaps = column.children.whereType<SizedBox>().toList();
      expect(colGaps, hasLength(1));
      expect(colGaps.single.height, 9);
      expect(colGaps.single.width, isNull);
    });

    testWidgets('spacing is skipped when children collapse to fewer than two', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'stack',
            'axis': 'column',
            'spacing': 9,
            'children': [
              {'kind': 'text', 'content': 'only'},
              {'kind': 'if', 'condition': 'never', 'thenChildren': []},
            ],
          },
        ]),
      );
      final column = tester.widget<Column>(_flexAround('only', Column).first);
      expect(column.children.whereType<SizedBox>(), isEmpty);
    });

    testWidgets('a progress bar inside a horizontal container still lays out', (
      tester,
    ) async {
      // A LinearProgressIndicator has no intrinsic width; a horizontal parent
      // hands its children unbounded width. This pairing used to fail layout.
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'stack',
            'axis': 'row',
            'children': [
              {'kind': 'progress', 'value': 1, 'max': 4},
              {'kind': 'meter', 'value': 1, 'min': 0, 'max': 4},
              {
                'kind': 'list',
                'children': [
                  {'kind': 'text', 'content': 'bulleted'},
                ],
              },
            ],
          },
        ]),
      );
      expect(tester.takeException(), isNull);
      expect(find.byType(LinearProgressIndicator), findsNWidgets(2));
      expect(find.text('bulleted'), findsOneWidget);
      expect(find.text('•'), findsOneWidget);
    });
  });

  group('control flow nested inside control flow and inside style', () {
    testWidgets('if inside forEach inside a styled, spaced stack', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(
          const [
            {
              'kind': 'stack',
              'axis': 'column',
              'spacing': 3,
              'style': {'padding': 6, 'opacity': 0.9},
              'children': [
                {
                  'kind': 'forEach',
                  'bind': 'rows',
                  'itemName': 'row',
                  'itemBody': [
                    {
                      'kind': 'if',
                      'condition': 'row.on',
                      'thenChildren': [
                        {'kind': 'text', 'content': 'ON {row.name}'},
                      ],
                      'elseChildren': [
                        {'kind': 'text', 'content': 'OFF {row.name}'},
                      ],
                    },
                  ],
                },
              ],
            },
          ],
          data: const {
            'rows': [
              {'name': 'a', 'on': true},
              {'name': 'b', 'on': false},
              {'name': 'c', 'on': 1},
              {'name': 'd', 'on': ''},
            ],
          },
        ),
      );
      expect(find.text('ON a'), findsOneWidget);
      expect(find.text('OFF b'), findsOneWidget);
      expect(find.text('ON c'), findsOneWidget);
      expect(find.text('OFF d'), findsOneWidget);
      expect(tester.widget<Opacity>(_under(Opacity)).opacity, 0.9);
    });

    testWidgets('forEach inside if inside a list keeps markers numbered', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(
          const [
            {
              'kind': 'list',
              'ordered': true,
              'children': [
                {
                  'kind': 'if',
                  'condition': 'show',
                  'thenChildren': [
                    {
                      'kind': 'forEach',
                      'bind': 'xs',
                      'itemName': 'x',
                      'itemBody': [
                        {'kind': 'text', 'content': '{x}'},
                      ],
                    },
                  ],
                },
                {'kind': 'text', 'content': 'tail'},
              ],
            },
          ],
          data: const {
            'show': true,
            'xs': ['p', 'q'],
          },
        ),
      );
      // The whole `if` is one list item, so the numbering is 1. then 2.
      expect(find.text('1.'), findsOneWidget);
      expect(find.text('2.'), findsOneWidget);
      expect(find.text('3.'), findsNothing);
      expect(find.text('p'), findsOneWidget);
      expect(find.text('q'), findsOneWidget);
      expect(find.text('tail'), findsOneWidget);
    });

    testWidgets('a list item that renders nothing does not consume a number', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'list',
            'ordered': true,
            'children': [
              {'kind': 'if', 'condition': 'nope', 'thenChildren': []},
              {'kind': 'text', 'content': 'first visible'},
            ],
          },
        ]),
      );
      expect(find.text('1.'), findsOneWidget);
      expect(find.text('2.'), findsNothing);
      expect(
        find.descendant(
          of: _flexAround('first visible', Row).first,
          matching: find.text('1.'),
        ),
        findsOneWidget,
      );
    });

    testWidgets('nested forEach shadowing resolves the innermost binding', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(
          const [
            {
              'kind': 'forEach',
              'bind': 'outer',
              'itemName': 'v',
              'itemBody': [
                {
                  'kind': 'forEach',
                  'bind': 'inner',
                  'itemName': 'v',
                  'itemBody': [
                    {'kind': 'text', 'content': 'v={v}'},
                  ],
                },
                {'kind': 'text', 'content': 'after={v}'},
              ],
            },
          ],
          data: const {
            'outer': ['O'],
            'inner': ['I1', 'I2'],
          },
        ),
      );
      expect(find.text('v=I1'), findsOneWidget);
      expect(find.text('v=I2'), findsOneWidget);
      // The outer binding is restored for the sibling, not clobbered.
      expect(find.text('after=O'), findsOneWidget);
    });

    testWidgets('an if whose branch is entirely unsupported renders nothing', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(
          const [
            {
              'kind': 'if',
              'condition': 'yes',
              'thenChildren': [
                {'kind': 'forEach', 'bind': 'missing', 'itemBody': []},
              ],
              'elseChildren': [
                {'kind': 'text', 'content': 'ELSE'},
              ],
            },
          ],
          data: const {'yes': true},
        ),
      );
      // The then-branch is taken and produces nothing; the else branch must
      // NOT be used as a fallback.
      expect(find.text('ELSE'), findsNothing);
    });
  });

  group('actions on nested interactive nodes', () {
    testWidgets('the innermost long-press wins the gesture arena', (
      tester,
    ) async {
      final fired = <String>[];
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'stack',
            'onLongPress': 'outer',
            'children': [
              {
                'kind': 'listItem',
                'onLongPress': 'middle',
                'children': [
                  {
                    'kind': 'stack',
                    'onLongPress': 'inner',
                    'children': [
                      {'kind': 'text', 'content': 'target'},
                    ],
                  },
                ],
              },
            ],
          },
        ], onAction: fired.add),
      );
      await tester.longPress(find.text('target'));
      expect(fired, ['inner']);
    });

    testWidgets('a tap and a long press on one button are separate seams', (
      tester,
    ) async {
      final fired = <String>[];
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'button',
            'label': 'B',
            'onClick': 'click-action',
            'onLongPress': 'press-action',
          },
        ], onAction: fired.add),
      );
      await tester.tap(find.text('B'));
      expect(fired, ['click-action']);
      fired.clear();
      await tester.longPress(find.text('B'));
      expect(fired, ['press-action']);
    });

    testWidgets('sibling controls dispatch independently and in order', (
      tester,
    ) async {
      final fired = <String>[];
      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'stack',
            'children': [
              {'kind': 'button', 'label': 'B1', 'onClick': 'a1'},
              {'kind': 'toggle', 'label': 'T', 'onChange': 'a2'},
              {'kind': 'checkbox', 'label': 'C', 'onChange': 'a3'},
              {'kind': 'button', 'label': 'B2', 'onClick': 'a4'},
            ],
          },
        ], onAction: fired.add),
      );
      await tester.tap(find.text('B2'));
      await tester.tap(find.byType(Checkbox));
      await tester.tap(find.byType(Switch));
      await tester.tap(find.text('B1'));
      expect(fired, ['a4', 'a3', 'a2', 'a1']);
    });

    testWidgets('a control inside a forEach dispatches the interpolated-free '
        'action once per item', (tester) async {
      final fired = <String>[];
      await tester.pumpWidget(
        _doc(
          const [
            {
              'kind': 'forEach',
              'bind': 'xs',
              'itemName': 'x',
              'itemBody': [
                {'kind': 'button', 'label': '{x}', 'onClick': 'do:{x}'},
              ],
            },
          ],
          data: const {
            'xs': ['a', 'b'],
          },
          onAction: fired.add,
        ),
      );
      // Labels ARE interpolated; actions are not.
      expect(find.text('{x}'), findsNWidgets(2));
      await tester.tap(find.byType(TextButton).first);
      await tester.tap(find.byType(TextButton).last);
      expect(fired, ['do:{x}', 'do:{x}']);
    });
  });

  group('theme and per-node style interact predictably', () {
    testWidgets('per-node colour beats the theme; the theme fills the gaps', (
      tester,
    ) async {
      const crepusTheme = CrepusTheme(
        textColor: Color(0xff010203),
        mutedColor: Color(0xff040506),
        accentColor: Color(0xff070809),
        borderColor: Color(0xff0a0b0c),
        surfaceColor: Color(0xff0d0e0f),
        baseFontSize: 21,
      );
      await tester.pumpWidget(
        _doc(const [
          {'kind': 'text', 'content': 'themed'},
          {
            'kind': 'text',
            'content': 'styled',
            'style': {'foregroundColor': '#ffffff'},
          },
          {'kind': 'progress', 'label': 'P', 'value': 1, 'max': 2},
          {'kind': 'badge', 'label': 'info', 'tone': 'info'},
          {'kind': 'badge', 'label': 'unknown', 'tone': 'chartreuse'},
          {'kind': 'divider'},
          {
            'kind': 'list',
            'children': [
              {'kind': 'text', 'content': 'item'},
            ],
          },
        ], crepusTheme: crepusTheme),
      );

      expect(
        tester.widget<Text>(find.text('themed')).style!.color,
        const Color(0xff010203),
      );
      expect(tester.widget<Text>(find.text('themed')).style!.fontSize, 21);
      expect(
        tester.widget<Text>(find.text('styled')).style!.color,
        const Color(0xffffffff),
      );
      // A muted label uses the theme's muted colour at baseFontSize - 2.
      final label = tester.widget<Text>(find.text('P'));
      expect(label.style!.color, const Color(0xff040506));
      expect(label.style!.fontSize, 19);
      // The accent drives the progress bar and the `info` badge tone.
      final bar = tester.widget<LinearProgressIndicator>(
        find.byType(LinearProgressIndicator),
      );
      expect(bar.value, 0.5);
      expect(bar.backgroundColor, const Color(0xff0a0b0c));
      expect(bar.valueColor!.value, const Color(0xff070809));
      expect(
        tester.widget<Text>(find.text('info')).style!.color,
        const Color(0xff070809),
      );
      // An unknown tone falls back to muted, not to the accent.
      expect(
        tester.widget<Text>(find.text('unknown')).style!.color,
        const Color(0xff040506),
      );
      expect(
        tester.widget<Divider>(find.byType(Divider)).color,
        const Color(0xff0a0b0c),
      );
      expect(
        tester.widget<Text>(find.text('•')).style!.color,
        const Color(0xff040506),
      );
    });

    testWidgets('with no CrepusTheme the ambient ThemeData supplies colours', (
      tester,
    ) async {
      final scheme = ColorScheme.fromSeed(seedColor: const Color(0xff123456));
      await tester.pumpWidget(
        _doc(const [
          {'kind': 'text', 'content': 'themed'},
          {'kind': 'progress', 'value': 1, 'max': 1},
        ], theme: ThemeData(colorScheme: scheme)),
      );
      expect(
        tester.widget<Text>(find.text('themed')).style!.color,
        scheme.onSurface,
      );
      expect(
        tester
            .widget<LinearProgressIndicator>(
              find.byType(LinearProgressIndicator),
            )
            .valueColor!
            .value,
        scheme.primary,
      );
    });

    testWidgets('a partially overridden theme keeps the ambient rest', (
      tester,
    ) async {
      final scheme = ColorScheme.fromSeed(seedColor: const Color(0xff654321));
      await tester.pumpWidget(
        _doc(
          const [
            {'kind': 'text', 'content': 'themed'},
            {'kind': 'badge', 'label': 'info', 'tone': 'info'},
          ],
          crepusTheme: const CrepusTheme(textColor: Color(0xffabcdef)),
          theme: ThemeData(colorScheme: scheme),
        ),
      );
      expect(
        tester.widget<Text>(find.text('themed')).style!.color,
        const Color(0xffabcdef),
      );
      expect(
        tester.widget<Text>(find.text('info')).style!.color,
        scheme.primary,
      );
    });
  });

  group('resolved values for the numeric leaves', () {
    testWidgets('progress and meter fractions are clamped into 0..1', (
      tester,
    ) async {
      const cases = <(double, double, double, double)>[
        (0, 100, 0, 0),
        (50, 100, 0, 0.5),
        (200, 100, 0, 1),
        (-5, 100, 0, 0),
        (1, 0, 0, 0),
        (5, 10, 5, 0),
        (7.5, 10, 5, 0.5),
        (99, 10, 5, 1),
        (5, 5, 5, 0),
        (5, 1, 10, 0),
      ];
      for (final (value, max, min, expected) in cases) {
        await tester.pumpWidget(
          _doc([
            {'kind': 'meter', 'value': value, 'min': min, 'max': max},
          ]),
        );
        expect(
          tester
              .widget<LinearProgressIndicator>(
                find.byType(LinearProgressIndicator),
              )
              .value,
          expected,
          reason: 'meter $value in $min..$max',
        );
      }
      for (final (value, max, _, expected) in cases.take(5)) {
        await tester.pumpWidget(
          _doc([
            {'kind': 'progress', 'value': value, 'max': max},
          ]),
        );
        expect(
          tester
              .widget<LinearProgressIndicator>(
                find.byType(LinearProgressIndicator),
              )
              .value,
          expected,
          reason: 'progress $value/$max',
        );
      }
    });

    testWidgets('a spacer resolves to a square box of the requested size', (
      tester,
    ) async {
      for (final pair in const [(null, 8.0), (0, 0.0), (-4, 0.0), (13, 13.0)]) {
        await tester.pumpWidget(
          _doc([
            {'kind': 'spacer', if (pair.$1 != null) 'size': pair.$1},
          ]),
        );
        final box = tester.widget<SizedBox>(_under(SizedBox).first);
        expect(box.width, pair.$2, reason: '${pair.$1}');
        expect(box.height, pair.$2, reason: '${pair.$1}');
      }
    });

    testWidgets('a divider picks its orientation from the axis', (
      tester,
    ) async {
      await tester.pumpWidget(
        _doc(const [
          {'kind': 'divider', 'axis': 'column'},
        ]),
      );
      expect(find.byType(Divider), findsOneWidget);
      expect(find.byType(VerticalDivider), findsNothing);

      await tester.pumpWidget(
        _doc(const [
          {
            'kind': 'stack',
            'axis': 'row',
            'children': [
              {'kind': 'divider', 'axis': 'row'},
            ],
          },
        ]),
      );
      expect(find.byType(VerticalDivider), findsOneWidget);
      expect(tester.widget<SizedBox>(_under(SizedBox).first).height, 16);
    });

    testWidgets('every badge tone resolves to its documented colour', (
      tester,
    ) async {
      const tones = {
        'positive': Color(0xff22c55e),
        'success': Color(0xff22c55e),
        'warning': Color(0xfff59e0b),
        'negative': Color(0xffef4444),
        'danger': Color(0xffef4444),
      };
      for (final entry in tones.entries) {
        await tester.pumpWidget(
          _doc([
            {'kind': 'badge', 'label': 'B', 'tone': entry.key},
          ]),
        );
        expect(
          tester.widget<Text>(find.text('B')).style!.color,
          entry.value,
          reason: entry.key,
        );
        final decoration =
            tester.widget<Container>(_under(Container).first).decoration
                as BoxDecoration;
        expect(
          decoration.color,
          entry.value.withValues(alpha: 0.14),
          reason: entry.key,
        );
      }
    });

    testWidgets('font weights bucket downward, never off the end', (
      tester,
    ) async {
      const cases = {
        -100: FontWeight.w300,
        0: FontWeight.w300,
        399: FontWeight.w300,
        400: FontWeight.w400,
        499: FontWeight.w400,
        500: FontWeight.w500,
        650: FontWeight.w600,
        700: FontWeight.w700,
        850: FontWeight.w800,
        100000: FontWeight.w800,
      };
      for (final entry in cases.entries) {
        await tester.pumpWidget(
          _doc([
            {
              'kind': 'text',
              'content': 'W',
              'style': {'fontWeight': entry.key},
            },
          ]),
        );
        expect(
          tester.widget<Text>(find.text('W')).style!.fontWeight,
          entry.value,
          reason: '${entry.key}',
        );
      }
    });

    testWidgets('colour parsing resolves each accepted spelling exactly', (
      tester,
    ) async {
      const cases = {
        '#ff0000': Color(0xffff0000),
        'ff0000': Color(0xffff0000),
        '  #00ff00  ': Color(0xff00ff00),
        // 8 digits are RRGGBBAA on the wire and rotate to AARRGGBB.
        '#11223344': Color(0x44112233),
        '00ff00ff': Color(0xff00ff00),
      };
      for (final entry in cases.entries) {
        await tester.pumpWidget(
          _doc([
            {
              'kind': 'text',
              'content': 'C',
              'style': {'foregroundColor': entry.key},
            },
          ]),
        );
        expect(
          tester.widget<Text>(find.text('C')).style!.color,
          entry.value,
          reason: entry.key,
        );
      }
    });
  });

  group('the .crepus front-end composes the same way', () {
    testWidgets('utility classes stack into one resolved style', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          CrepusView.fromSource(
            'p px-2 pt-1 text-xl font-bold italic underline line-through '
            'text-center text-red-500 bg-blue-900 rounded-lg "Hi"',
          ),
        ),
      );
      final text = tester.widget<Text>(find.text('Hi'));
      expect(text.style!.fontSize, 20);
      expect(text.style!.fontWeight, FontWeight.w700);
      expect(text.style!.color, const Color(0xffef4444));
      expect(text.style!.fontStyle, FontStyle.italic);
      expect(text.textAlign, TextAlign.center);
      final decoration =
          tester.widget<DecoratedBox>(_under(DecoratedBox)).decoration
              as BoxDecoration;
      expect(decoration.color, const Color(0xff1e3a8a));
      expect(decoration.borderRadius, BorderRadius.circular(12));
      final padding = tester.widget<Padding>(
        find
            .ancestor(of: find.text('Hi'), matching: find.byType(Padding))
            .first,
      );
      expect(padding.padding, const EdgeInsets.only(top: 4, left: 8, right: 8));
    });

    testWidgets('layout classes, control flow and style compose in source', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(
          CrepusView.fromSource(
            '''
view p-2 gap-2
  stack row items-center justify-between gap-3
    text "left"
    badge "b" tone=warning
  for row in rows
    if row.on
      text "ON {row.name}"
    else
      text "OFF {row.name}"
''',
            data: const {
              'rows': [
                {'name': 'a', 'on': true},
                {'name': 'b', 'on': false},
              ],
            },
          ),
        ),
      );
      final row = tester.widget<Row>(_flexAround('left', Row).first);
      expect(row.crossAxisAlignment, CrossAxisAlignment.center);
      expect(row.mainAxisAlignment, MainAxisAlignment.spaceBetween);
      expect(row.children.whereType<SizedBox>().single.width, 12);
      expect(find.text('ON a'), findsOneWidget);
      expect(find.text('OFF b'), findsOneWidget);
      expect(
        tester.widget<Text>(find.text('b')).style!.color,
        const Color(0xfff59e0b),
      );
    });
  });
}
