import 'dart:ui' as ui;
import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final kind in ['toggle', 'checkbox']) {
    for (final enabled in [true, false]) {
      testWidgets(
        '$kind exposes its label with its ${enabled ? 'enabled' : 'disabled'} state',
        (tester) async {
          final semantics = tester.ensureSemantics();
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
                        'label': 'Accept terms',
                        'checked': true,
                        if (enabled) 'onChange': 'accept',
                        if (kind == 'toggle') 'onLongPress': 'held',
                      },
                    ],
                  }, onAction: actions.add),
                ),
              ),
            );
            final nodes = <SemanticsNode>[];
            void collect(SemanticsNode node) {
              nodes.add(node);
              node.visitChildren((child) {
                collect(child);
                return true;
              });
            }

            final owner =
                tester.binding.renderViews.single.owner!.semanticsOwner!;
            collect(owner.rootSemanticsNode!);
            final control = nodes.singleWhere(
              (node) => node.getSemanticsData().label == 'Accept terms',
            );
            final data = control.getSemanticsData();
            expect(
              control,
              matchesSemantics(
                label: 'Accept terms',
                hasEnabledState: true,
                isEnabled: enabled,
                hasCheckedState: kind == 'checkbox',
                isChecked: kind == 'checkbox',
                hasToggledState: kind == 'toggle',
                isToggled: kind == 'toggle',
                isFocusable: enabled,
                hasTapAction: enabled,
                hasFocusAction: enabled,
                hasLongPressAction: kind == 'toggle',
              ),
            );
            if (enabled) {
              owner.performAction(control.id, ui.SemanticsAction.tap);
              await tester.pump();
              expect(actions, ['accept']);
            }
            if (kind == 'toggle') {
              expect(data.hasAction(ui.SemanticsAction.longPress), isTrue);
              owner.performAction(control.id, ui.SemanticsAction.longPress);
              await tester.pump();
              expect(actions, [if (enabled) 'accept', 'held']);
            }
            expect(tester.takeException(), isNull);
          } finally {
            semantics.dispose();
          }
        },
      );
    }
  }
}
