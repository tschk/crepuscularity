import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Widget host(
  List<Map<String, Object?>> nodes,
  Map<String, Object?> data,
  List<String> actions,
) => MaterialApp(
  home: CrepusView.fromIr(
    {'version': IR_VERSION, 'root': nodes},
    data: data,
    allowNetworkImages: true,
    onAction: actions.add,
  ),
);

Map<String, Object?> link(
  String label, {
  String? bind,
  String href = '/static',
}) => {
  'kind': 'link',
  'href': href,
  if (bind != null) 'hrefBind': bind,
  'children': [
    {'kind': 'text', 'content': label},
  ],
};

List<String> imageUrls(WidgetTester tester) => tester
    .widgetList<Image>(find.byType(Image))
    .map((image) => (image.image as NetworkImage).url)
    .toList();

void main() {
  testWidgets(
    'root URLs update without interpolating resolved braces or escapes',
    (tester) async {
      final actions = <String>[];
      final nodes = [
        {
          'kind': 'image',
          'src': 'https://example.com/snapshot',
          'srcBind': 'photo',
        },
        link('open', bind: 'destination'),
        link('static', href: r'/literal/{photo}\path'),
      ];
      for (final value in [r'/one/{literal}\path?x="a"&y=2', '/two\nline']) {
        await tester.pumpWidget(
          host(nodes, {
            'photo': 'https://example.com$value',
            'destination': value,
          }, actions),
        );
        expect(imageUrls(tester), ['https://example.com$value']);
        await tester.tap(find.text('open'));
        expect(actions.last, value);
        await tester.tap(find.text('static'));
        expect(actions.last, r'/literal/{photo}\path');
      }
    },
  );

  testWidgets('loop URLs use each row and refresh link callbacks', (
    tester,
  ) async {
    final actions = <String>[];
    final nodes = <Map<String, Object?>>[
      {
        'kind': 'forEach',
        'bind': 'rows',
        'itemName': 'row',
        'itemBody': [
          {'kind': 'image', 'src': '', 'srcBind': 'row.src'},
          {
            'kind': 'link',
            'href': '',
            'hrefBind': 'row.href',
            'children': [
              {'kind': 'text', 'content': '{row.label}'},
            ],
          },
        ],
      },
    ];
    for (final suffix in ['first', 'updated']) {
      await tester.pumpWidget(
        host(nodes, {
          'rows': [
            {
              'src': 'https://example.com/a/$suffix',
              'href': '/a/$suffix',
              'label': 'A',
            },
            {
              'src': 'https://example.com/b/$suffix',
              'href': '/b/$suffix',
              'label': 'B',
            },
          ],
        }, actions),
      );
      expect(imageUrls(tester), [
        'https://example.com/a/$suffix',
        'https://example.com/b/$suffix',
      ]);
      await tester.tap(find.text('A'));
      await tester.tap(find.text('B'));
      expect(actions.sublist(actions.length - 2), ['/a/$suffix', '/b/$suffix']);
    }
  });

  testWidgets(
    'missing/null URLs empty; supported expressions and snapshot fallback',
    (tester) async {
      final actions = <String>[];
      await tester.pumpWidget(
        host(
          [
            {
              'kind': 'image',
              'src': 'https://example.com/snapshot',
              'srcBind': 'missing',
            },
            link('missing', bind: 'missing'),
            link('null', bind: 'value'),
            link('literal', bind: r'"/quoted\\path"'),
            link('mixed', bind: r'"" + "/item/" + (id) + "?raw={x}"'),
            link('unsupported', bind: 'id + 1', href: '/snapshot'),
          ],
          {'value': null, 'id': 7},
          actions,
        ),
      );
      expect(find.byType(Image), findsNothing);
      await tester.tap(find.text('missing'));
      await tester.tap(find.text('null'));
      expect(actions, isEmpty);
      await tester.tap(find.text('literal'));
      await tester.tap(find.text('mixed'));
      await tester.tap(find.text('unsupported'));
      expect(actions, [r'/quoted\path', '/item/7?raw={x}', '/snapshot']);
    },
  );
}
