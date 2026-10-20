import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final kind in ['link', 'listItem']) {
    for (final signed in [false, true]) {
      testWidgets('$kind keyboard activation with signed margins=$signed', (
        tester,
      ) async {
        final actions = <String>[];
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: CrepusView.fromIr({
                'version': IR_VERSION,
                'root': [
                  {
                    'kind': kind,
                    if (kind == 'link') 'href': 'open' else 'onClick': 'open',
                    if (signed) 'style': {'marginHorizontal': -4.0},
                    'children': [
                      {'kind': 'text', 'content': 'Open item'},
                    ],
                  },
                ],
              }, onAction: actions.add),
            ),
          ),
        );
        await tester.sendKeyEvent(LogicalKeyboardKey.tab);
        await tester.pump();
        for (final key in [
          LogicalKeyboardKey.enter,
          LogicalKeyboardKey.space,
          LogicalKeyboardKey.numpadEnter,
        ]) {
          await tester.sendKeyEvent(key);
          await tester.pump();
        }
        expect(actions, ['open', 'open', 'open']);
        final outlines = find.byWidgetPredicate(
          (widget) =>
              widget is DecoratedBox &&
              widget.position == DecorationPosition.foreground &&
              widget.decoration is BoxDecoration &&
              (widget.decoration as BoxDecoration).border?.top.width == 2,
        );
        expect(outlines, findsOneWidget);
        await tester.tap(find.text('Open item'));
        expect(actions, ['open', 'open', 'open', 'open']);
        FocusManager.instance.primaryFocus?.unfocus();
        await tester.pumpAndSettle();
        expect(outlines, findsNothing);
        expect(tester.takeException(), isNull);
      });
    }
  }
  testWidgets('nested button keyboard action does not activate its parent', (
    tester,
  ) async {
    final actions = <String>[];
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: CrepusView.fromIr({
            'version': IR_VERSION,
            'root': [
              {
                'kind': 'listItem',
                'onClick': 'parent',
                'children': [
                  {'kind': 'button', 'label': 'Child', 'onClick': 'child'},
                ],
              },
            ],
          }, onAction: actions.add),
        ),
      ),
    );
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.sendKeyEvent(LogicalKeyboardKey.tab);
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();
    expect(actions, ['child']);
  });
  for (final platform in [
    TargetPlatform.macOS,
    TargetPlatform.windows,
    TargetPlatform.linux,
    null,
  ]) {
    for (final kind in ['button', 'toggle', 'checkbox']) {
      testWidgets(
        'nested $kind retains ${platform ?? 'web'} keyboard defaults',
        (tester) async {
          for (final key in [
            LogicalKeyboardKey.enter,
            LogicalKeyboardKey.space,
            LogicalKeyboardKey.numpadEnter,
          ]) {
            final observed = <List<String>>[];
            for (final nested in [false, true]) {
              final actions = <String>[];
              final child = <String, Object?>{
                'kind': kind,
                'label': 'Child',
                if (kind == 'button')
                  'onClick': 'child'
                else
                  'onChange': 'child',
              };
              await tester.pumpWidget(const SizedBox.shrink());
              await tester.pumpWidget(
                MaterialApp(
                  shortcuts: platform == null
                      ? {
                          ...WidgetsApp.defaultShortcuts,
                          const SingleActivator(LogicalKeyboardKey.enter):
                              const ButtonActivateIntent(),
                          const SingleActivator(LogicalKeyboardKey.numpadEnter):
                              const ButtonActivateIntent(),
                        }
                      : null,
                  home: Scaffold(
                    body: CrepusView.fromIr({
                      'version': IR_VERSION,
                      'root': [
                        if (nested)
                          {
                            'kind': 'listItem',
                            'onClick': 'parent',
                            'children': [child],
                          }
                        else
                          child,
                      ],
                    }, onAction: actions.add),
                  ),
                ),
              );
              await tester.sendKeyEvent(LogicalKeyboardKey.tab);
              if (nested) await tester.sendKeyEvent(LogicalKeyboardKey.tab);
              await tester.pump();
              // Linux event data supports numpad Enter under every host policy.
              await tester.sendKeyEvent(key, platform: 'linux');
              await tester.pump();
              observed.add(actions);
            }
            expect(
              observed.last,
              observed.first,
              reason: '$platform $kind $key',
            );
          }
        },
        variant: TargetPlatformVariant({platform ?? TargetPlatform.linux}),
      );
    }
  }
}
