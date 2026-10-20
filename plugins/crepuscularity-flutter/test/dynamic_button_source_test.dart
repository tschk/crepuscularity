import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('source preserves raw expressions and decodes only literal escapes', () {
    final node =
        viewIrFromSource(
              r'''button "Line\nC:\\ {row.name} {'}'}"''',
            ).root.single
            as ButtonNode;
    expect(node.labelParts.map((p) => (p.kind, p.value)).toList(), [
      ('literal', 'Line\nC:\\ '),
      ('expr', 'row.name'),
      ('literal', ' '),
      ('expr', "'}'"),
    ]);
    final escapedExpression =
        viewIrFromSource(r'''button "{'\n'}"''').root.single as ButtonNode;
    expect(escapedExpression.labelParts.single.value, r"'\n'");
    final nested =
        viewIrFromSource('button\n  "Hello {name}"').root.single as ButtonNode;
    expect(nested.labelParts.last.value, 'name');
  });

  test('double quoted expression values retain spaces and closing braces', () {
    final node =
        viewIrFromSource(r'''button "A {"quoted } value"} B"''').root.single
            as ButtonNode;
    expect(node.labelParts.map((part) => (part.kind, part.value)).toList(), [
      ('literal', 'A '),
      ('expr', '"quoted } value"'),
      ('literal', ' B'),
    ]);
  });

  testWidgets('quoted expression literals render their complete value', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: CrepusView.fromSource(r'''button "A {"quoted } value"} B"'''),
      ),
    );
    expect(find.text('A quoted } value B'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  test('canonical event prefix preserves normalized long press aliases', () {
    for (final event in [
      '@longpress',
      '@long-press',
      'onlongpress',
      'on-long-press',
    ]) {
      final node =
          viewIrFromSource('button $event=hold "Hold"').root.single
              as ButtonNode;
      expect(node.onLongPress, 'hold');
    }
  });

  test('source falls back safely and label attributes retain precedence', () {
    for (final source in [
      'button "No expressions"',
      'button "Missing {name"',
      'button "Empty {}"',
      'button "Only quote',
      'button label="Fixed" "Dynamic {name}"',
      'button label="Fixed"\n  "Dynamic {name}"',
    ]) {
      final node = viewIrFromSource(source).root.single as ButtonNode;
      expect(node.labelParts, isEmpty, reason: source);
      if (source.contains('label=')) expect(node.label, 'Fixed');
    }
    expect(
      viewIrFromSource(
        'button "Hello {name}"',
        limits: const CrepusLimits(maxNodes: 2),
      ).root,
      isEmpty,
    );
  });

  test('quoted action payload braces remain opaque', () {
    final node =
        viewIrFromSource(
              'button onclick="prompt:{" onlongpress=hold "Choose"',
            ).root.single
            as ButtonNode;
    expect(node.onClick, 'prompt:{');
    expect(node.onLongPress, 'hold');
    expect(node.label, 'Choose');
  });

  testWidgets('literal attribute braces preserve following action tokens', (
    tester,
  ) async {
    for (final event in ['@click', 'onclick']) {
      final actions = <String>[];
      await tester.pumpWidget(
        MaterialApp(
          home: CrepusView.fromSource(
            'button label="{" $event=choose',
            onAction: (action) => actions.add(action),
          ),
        ),
      );
      expect(find.text('{'), findsOneWidget);
      await tester.tap(find.text('{'));
      expect(actions, ['choose']);
    }
  });

  testWidgets('source loop labels reflect host updates and keep actions', (
    tester,
  ) async {
    for (final childLabel in [false, true]) {
      final actions = <String>[];
      Future<void> render(String name) => tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: CrepusView.fromSource(
              childLabel
                  ? 'for row in rows\n  button onclick=choose\n    "Choose {row.name}"'
                  : 'for row in rows\n  button @click=choose "Choose {row.name}"',
              data: {
                'rows': [
                  {'name': name},
                ],
              },
              onAction: (action) => actions.add(action),
            ),
          ),
        ),
      );
      await render('Ada {secret}');
      expect(find.text('Choose Ada {secret}'), findsOneWidget);
      await tester.tap(find.text('Choose Ada {secret}'));
      expect(actions, ['choose']);
      await render('Grace');
      expect(find.text('Choose Grace'), findsOneWidget);
      expect(find.text('Choose Ada {secret}'), findsNothing);
      expect(tester.takeException(), isNull);
    }
  });
}
