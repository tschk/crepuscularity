import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  for (final signed in [false, true]) {
    for (final direction in TextDirection.values) {
      testWidgets('narrow list bounds its marker ($signed, $direction)', (
        tester,
      ) async {
        const hostKey = ValueKey('list-host');
        for (final width in [0.0, 8.0, 200.0]) {
          for (final ordered in [false, true]) {
            await tester.pumpWidget(
              MaterialApp(
                home: Directionality(
                  textDirection: direction,
                  child: Align(
                    alignment: Alignment.topLeft,
                    child: SizedBox(
                      key: hostKey,
                      width: width,
                      height: 500,
                      child: CrepusView.fromIr({
                        'version': IR_VERSION,
                        'root': [
                          {
                            'kind': 'list',
                            'ordered': ordered,
                            'children': [
                              for (var item = 0; item < 12; item++)
                                {
                                  'kind': 'text',
                                  'content': 'x',
                                  if (signed) 'style': {'marginTop': -1},
                                },
                            ],
                          },
                        ],
                      }),
                    ),
                  ),
                ),
              ),
            );
            expect(tester.takeException(), isNull);
            final host = tester.getRect(find.byKey(hostKey));
            final marker = tester.getRect(
              find.text(ordered ? '12.' : '•').last,
            );
            expect(marker.left, greaterThanOrEqualTo(host.left));
            expect(marker.right, lessThanOrEqualTo(host.right));
            if (signed && ordered && width == 8) {
              final box = tester.renderObject<RenderBox>(
                find.byType(CrepusView),
              );
              final dry = box.getDryLayout(BoxConstraints(maxWidth: width));
              expect(
                box.getMaxIntrinsicHeight(width),
                closeTo(dry.height, 0.001),
              );
              expect(
                box.getMinIntrinsicHeight(width),
                closeTo(dry.height, 0.001),
              );
            }
          }
        }
      });
    }
  }
}
