import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Map<String, Object?> scroll(String axis) => {
  'kind': 'scroll',
  'axis': axis,
  'children': [
    for (var i = 0; i < 20; i++) {'kind': 'spacer', 'size': 20},
    {'kind': 'button', 'label': 'End', 'onClick': 'end'},
  ],
};
Widget host(List<Map<String, Object?>> root, {void Function(String)? action}) =>
    MaterialApp(
      home: Scaffold(
        body: Align(
          alignment: Alignment.topLeft,
          child: SizedBox(
            width: 200,
            height: 120,
            child: CrepusView.fromIr({
              'version': IR_VERSION,
              'root': root,
            }, onAction: action),
          ),
        ),
      ),
    );
void main() {
  for (final axis in ['column', 'row']) {
    for (final placement in ['root', 'column', 'row', 'if', 'list', 'signed']) {
      testWidgets('$axis scroll is bounded through $placement', (tester) async {
        final node = scroll(axis);
        final Map<String, Object?> root = switch (placement) {
          'root' => node,
          'signed' => {
            ...node,
            'style': {'margin': -4.0, 'padding': 4.0},
          },
          'if' => {
            'kind': 'if',
            'condition': 'true',
            'thenChildren': [node],
          },
          'list' => {
            'kind': 'list',
            'children': [node],
          },
          _ => {
            'kind': 'stack',
            'axis': placement,
            'children': [node],
          },
        };
        final actions = <String>[];
        await tester.pumpWidget(host([root], action: actions.add));
        expect(tester.takeException(), isNull);
        final viewport = find.byType(SingleChildScrollView);
        expect(viewport, findsOneWidget);
        final size = tester.getSize(viewport);
        expect(size.width, lessThanOrEqualTo(200));
        expect(size.height, lessThanOrEqualTo(120));
        final state = tester.state<ScrollableState>(find.byType(Scrollable));
        expect(state.position.maxScrollExtent, greaterThan(0));
        await tester.drag(
          viewport,
          axis == 'row' ? const Offset(-600, 0) : const Offset(0, -600),
        );
        await tester.pumpAndSettle();
        await tester.tap(find.text('End'));
        await tester.pump();
        expect(actions, ['end']);
        expect(tester.takeException(), isNull);
      });
    }
  }
  testWidgets('fixed header and two scrolls share remaining height', (
    tester,
  ) async {
    await tester.pumpWidget(
      host([
        {'kind': 'spacer', 'size': 20},
        scroll('column'),
        scroll('column'),
      ]),
    );
    expect(tester.takeException(), isNull);
    final views = find.byType(SingleChildScrollView);
    expect(views, findsNWidgets(2));
    for (var i = 0; i < 2; i++) {
      expect(tester.getSize(views.at(i)).height, 50);
    }
  });
  testWidgets('unbounded host keeps natural scroll content size', (
    tester,
  ) async {
    await tester.pumpWidget(
      MaterialApp(
        home: SingleChildScrollView(
          child: CrepusView.fromIr({
            'version': IR_VERSION,
            'root': [scroll('column')],
          }),
        ),
      ),
    );
    expect(tester.takeException(), isNull);
    expect(
      tester.getSize(find.byType(SingleChildScrollView).last).height,
      greaterThan(400),
    );
  });
}
