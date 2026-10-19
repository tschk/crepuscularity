import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Future<void> showSource(
  WidgetTester tester,
  String source,
  Map<String, Object?> data,
  List<String> actions,
) => tester.pumpWidget(
  MaterialApp(
    home: CrepusView.fromSource(
      source,
      data: data,
      allowNetworkImages: true,
      onAction: actions.add,
    ),
  ),
);

List<String> urls(WidgetTester tester) => tester
    .widgetList<Image>(find.byType(Image))
    .map((image) => (image.image as NetworkImage).url)
    .toList();

void main() {
  test('last URL attribute replaces stale expression metadata', () {
    final ir = viewIrFromSource('''
image src={photo} src="/fixed.png"
image src="/fixed.png" src={photo}
link href={destination} href="/{literal}" "Static"
link href="/fixed" href={destination} "Dynamic"
''');
    expect((ir.root[0] as ImageNode).srcBind, isNull);
    expect((ir.root[0] as ImageNode).src, '/fixed.png');
    expect((ir.root[1] as ImageNode).srcBind, 'photo');
    expect((ir.root[2] as LinkNode).hrefBind, isNull);
    expect((ir.root[2] as LinkNode).href, '/{literal}');
    expect((ir.root[3] as LinkNode).hrefBind, 'destination');
  });

  testWidgets('source URL expressions update and quoted braces stay literal', (
    tester,
  ) async {
    final actions = <String>[];
    const source = '''
stack
  image src={photo}
  link href={destination} "Open"
  link href="/{destination}" "Literal"
''';
    for (final value in ['/one', '/two?x={literal}&y="quoted"']) {
      await showSource(tester, source, {
        'photo': 'https://example.com$value',
        'destination': value,
      }, actions);
      expect(urls(tester), ['https://example.com$value']);
      await tester.tap(find.text('Open'));
      expect(actions.last, value);
      await tester.tap(find.text('Literal'));
      expect(actions.last, '/{destination}');
    }
  });

  testWidgets('source loop URL bindings use current row values', (
    tester,
  ) async {
    final actions = <String>[];
    const source = '''
for row in rows
  image src={row.photo}
  link href={row.href} "{row.label}"
''';
    for (final revision in ['old', 'new']) {
      await showSource(tester, source, {
        'rows': [
          {
            'photo': 'https://example.com/a/$revision',
            'href': '/a/$revision',
            'label': 'A',
          },
          {
            'photo': 'https://example.com/b/$revision',
            'href': '/b/$revision',
            'label': 'B',
          },
        ],
      }, actions);
      expect(urls(tester), [
        'https://example.com/a/$revision',
        'https://example.com/b/$revision',
      ]);
      await tester.tap(find.text('A'));
      await tester.tap(find.text('B'));
      expect(actions.sublist(actions.length - 2), [
        '/a/$revision',
        '/b/$revision',
      ]);
    }
  });

  testWidgets('only a complete outer expression becomes a URL binding', (
    tester,
  ) async {
    final actions = <String>[];
    await showSource(
      tester,
      '''
stack
  link href={missing} "Missing"
  link href={destination}/tail "Incomplete"
  link href={"/quoted"} "Expression literal"
''',
      {'destination': '/live'},
      actions,
    );
    await tester.tap(find.text('Missing'));
    expect(actions, isEmpty);
    await tester.tap(find.text('Incomplete'));
    await tester.tap(find.text('Expression literal'));
    expect(actions, ['{destination}/tail', '/quoted']);
  });
}
