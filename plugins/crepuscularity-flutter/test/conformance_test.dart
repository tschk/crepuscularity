import 'dart:convert';
import 'dart:io';

import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// A normalized structural summary that ignores incidental style (e.g.
/// `flexDirection`) and interpolated text, so `fromSource` and `fromIr` can be
/// compared on the shared golden fixture.
Object _shape(ViewNode node) {
  final children = childrenOf(node).map(_shape).toList();
  return switch (node) {
    StackNode(:final axis, :final spacing) => {
      'kind': 'stack',
      'axis': axis.name,
      'spacing': spacing,
      'children': children,
    },
    TextNode() => {'kind': 'text'},
    _ => {'kind': node.runtimeType.toString(), 'children': children},
  };
}

List<Object> _shapeAll(List<ViewNode> nodes) => nodes.map(_shape).toList();

Widget _host(Widget child) => MaterialApp(home: Scaffold(body: child));

void main() {
  final fixtureJson =
      jsonDecode(File('test/fixtures/fixture.json').readAsStringSync())
          as Map<String, Object?>;
  final source = File('test/fixtures/main.crepus').readAsStringSync();

  testWidgets('named font sizes match the Rust IR ramp', (tester) async {
    final source = File('test/fixtures/font-sizes.crepus').readAsStringSync();
    final ir =
        jsonDecode(File('test/fixtures/font-sizes.json').readAsStringSync())
            as Map<String, Object?>;
    final expected = <String, double>{
      'xs': 12,
      'sm': 14,
      'base': 16,
      'lg': 18,
      'xl': 20,
      '2xl': 24,
      '3xl': 30,
      '4xl': 36,
      '5xl': 48,
      '6xl': 60,
      '7xl': 72,
      '8xl': 96,
      '9xl': 128,
    };
    for (final view in [CrepusView.fromSource(source), CrepusView.fromIr(ir)]) {
      await tester.pumpWidget(_host(SingleChildScrollView(child: view)));
      for (final entry in expected.entries) {
        expect(
          tester.widget<Text>(find.text(entry.key)).style!.fontSize,
          entry.value,
        );
      }
      expect(tester.takeException(), isNull);
    }
  });

  test('fromSource and fromIr agree structurally on the golden fixture', () {
    final fromIr = ViewIr.fromJson(fixtureJson);
    final fromSource = viewIrFromSource(source);
    expect(_shapeAll(fromSource.root), _shapeAll(fromIr.root));
  });

  testWidgets('fromSource and fromIr render the same visible text', (
    tester,
  ) async {
    // The Rust reference resolved `name = Ada` at lower time; the Dart parser
    // defers interpolation to render, so pass the same context via `data`.
    await tester.pumpWidget(_host(CrepusView.fromIr(fixtureJson)));
    expect(find.text('Hello Ada'), findsOneWidget);

    await tester.pumpWidget(
      _host(CrepusView.fromSource(source, data: const {'name': 'Ada'})),
    );
    expect(find.text('Hello Ada'), findsOneWidget);
  });
}
