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
