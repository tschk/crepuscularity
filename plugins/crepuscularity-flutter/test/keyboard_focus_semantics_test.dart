import 'dart:ui' as ui;
import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final kind in ['link', 'listItem']) {
    for (final signed in [false, true]) {
      testWidgets(
        '$kind exposes focus and activation with signed margins=$signed',
        (tester) async {
          final handle = tester.ensureSemantics();
          try {
            final actions = <String>[];
            await tester.pumpWidget(
              MaterialApp(
                home: Scaffold(
                  body: CrepusView.fromIr({
                    'version': IR_VERSION,
                    'root': [
                      {
                        'kind': kind,
                        if (kind == 'link')
                          'href': 'open'
                        else
                          'onClick': 'open',
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
            final owner =
                tester.binding.renderViews.single.owner!.semanticsOwner!;
            SemanticsNode item() {
              SemanticsNode? found;
              void visit(SemanticsNode node) {
                if (node.getSemanticsData().label == 'Open item') found = node;
                node.visitChildren((child) {
                  visit(child);
                  return true;
                });
              }

              visit(owner.rootSemanticsNode!);
              return found!;
            }

            expect(
              item(),
              matchesSemantics(
                label: 'Open item',
                isLink: kind == 'link',
                isFocusable: true,
                hasFocusAction: true,
                hasTapAction: true,
              ),
            );
            owner.performAction(item().id, ui.SemanticsAction.focus);
            await tester.pumpAndSettle();
            expect(
              item(),
              matchesSemantics(
                label: 'Open item',
                isLink: kind == 'link',
                isFocusable: true,
                isFocused: true,
                hasFocusAction: true,
                hasTapAction: true,
              ),
            );
            owner.performAction(item().id, ui.SemanticsAction.tap);
            await tester.pump();
            expect(actions, ['open']);
            FocusManager.instance.primaryFocus?.unfocus();
            await tester.pumpAndSettle();
            expect(
              item(),
              matchesSemantics(
                label: 'Open item',
                isLink: kind == 'link',
                isFocusable: true,
                hasFocusAction: true,
                hasTapAction: true,
              ),
            );
            expect(tester.takeException(), isNull);
          } finally {
            handle.dispose();
          }
        },
      );
    }
  }
  testWidgets('excluded focus does not advertise a focusable link', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    try {
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: ExcludeFocus(
              child: CrepusView.fromIr({
                'version': IR_VERSION,
                'root': [
                  {
                    'kind': 'link',
                    'href': 'open',
                    'children': [
                      {'kind': 'text', 'content': 'Excluded'},
                    ],
                  },
                ],
              }),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(
        tester.getSemantics(find.text('Excluded')),
        matchesSemantics(label: 'Excluded', isLink: true, hasTapAction: true),
      );
    } finally {
      handle.dispose();
    }
  });
  testWidgets(
    'iOS retains the native accessibility focus-action guard',
    (tester) async {
      final handle = tester.ensureSemantics();
      try {
        await tester.pumpWidget(
          MaterialApp(
            home: Scaffold(
              body: CrepusView.fromIr({
                'version': IR_VERSION,
                'root': [
                  {
                    'kind': 'link',
                    'href': 'open',
                    'children': [
                      {'kind': 'text', 'content': 'iOS link'},
                    ],
                  },
                ],
              }),
            ),
          ),
        );
        expect(
          tester.getSemantics(find.text('iOS link')),
          matchesSemantics(
            label: 'iOS link',
            isLink: true,
            isFocusable: true,
            hasTapAction: true,
          ),
        );
      } finally {
        handle.dispose();
      }
    },
    variant: TargetPlatformVariant({TargetPlatform.iOS}),
  );
  testWidgets('changing ExcludeFocus updates an existing link semantics', (
    tester,
  ) async {
    final handle = tester.ensureSemantics();
    try {
      Widget host(bool excluded) => MaterialApp(
        home: Scaffold(
          body: ExcludeFocus(
            excluding: excluded,
            child: CrepusView.fromIr({
              'version': IR_VERSION,
              'root': [
                {
                  'kind': 'link',
                  'href': 'open',
                  'children': [
                    {'kind': 'text', 'content': 'Dynamic link'},
                  ],
                },
              ],
            }),
          ),
        ),
      );
      for (final excluded in [false, true, false]) {
        await tester.pumpWidget(host(excluded));
        await tester.pumpAndSettle();
        expect(
          tester.getSemantics(find.text('Dynamic link')),
          matchesSemantics(
            label: 'Dynamic link',
            isLink: true,
            isFocusable: !excluded,
            hasFocusAction: !excluded,
            hasTapAction: true,
          ),
        );
      }
    } finally {
      handle.dispose();
    }
  });
  testWidgets(
    'link role updates without leaking to list items or inert links',
    (tester) async {
      final handle = tester.ensureSemantics();
      try {
        for (final kind in ['link', 'listItem', 'link', 'inert']) {
          await tester.pumpWidget(
            MaterialApp(
              home: Scaffold(
                body: CrepusView.fromIr({
                  'version': IR_VERSION,
                  'root': [
                    {
                      'kind': kind == 'inert' ? 'link' : kind,
                      if (kind == 'link') 'href': 'open',
                      if (kind == 'listItem') 'onClick': 'open',
                      'children': [
                        {'kind': 'text', 'content': 'Changing'},
                      ],
                    },
                  ],
                }),
              ),
            ),
          );
          await tester.pumpAndSettle();
          expect(
            tester.getSemantics(find.text('Changing')),
            matchesSemantics(
              label: 'Changing',
              isLink: kind == 'link',
              isFocusable: kind != 'inert',
              hasFocusAction: kind != 'inert',
              hasTapAction: kind != 'inert',
            ),
          );
        }
      } finally {
        handle.dispose();
      }
    },
  );
}
