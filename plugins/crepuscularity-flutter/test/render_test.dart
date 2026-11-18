import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Widget _host(Widget child) => MaterialApp(home: Scaffold(body: child));

void main() {
  testWidgets('renders text, button, badge, progress', (tester) async {
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource('''
stack col gap-2
  text "Reminder"
  badge "New" tone=positive
  progress label="Setup" value=50 max=100
  button "Do it" onclick=complete
'''),
      ),
    );
    expect(find.text('Reminder'), findsOneWidget);
    expect(find.text('New'), findsOneWidget);
    expect(find.text('Setup'), findsOneWidget);
    expect(find.widgetWithText(TextButton, 'Do it'), findsOneWidget);
    expect(find.byType(LinearProgressIndicator), findsOneWidget);
  });

  testWidgets('button click surfaces the raw action string', (tester) async {
    final actions = <String>[];
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource(
          'button "Go" onclick={prompt:Draft it}',
          onAction: actions.add,
        ),
      ),
    );
    await tester.tap(find.text('Go'));
    expect(actions, ['prompt:Draft it']);
  });

  testWidgets('toggle change surfaces its action', (tester) async {
    final actions = <String>[];
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource(
          'toggle "Flag" onchange=complete',
          onAction: actions.add,
        ),
      ),
    );
    await tester.tap(find.byType(Switch));
    expect(actions, ['complete']);
  });

  testWidgets('disallowed kind renders no visible content in release-like tree', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource('''
stack col
  webview src=https://evil.example
  text "safe"
'''),
      ),
    );
    // The safe sibling still renders; the webview produces no interactive widget.
    expect(find.text('safe'), findsOneWidget);
    expect(find.byType(WebViewMarker), findsNothing);
  });

  testWidgets('if renders the true branch from data scope', (tester) async {
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource(
          '''
if done
  text "finished"
else
  text "pending"
''',
          data: const {'done': true},
        ),
      ),
    );
    expect(find.text('finished'), findsOneWidget);
    expect(find.text('pending'), findsNothing);
  });

  testWidgets('forEach expands over a data list with interpolation', (
    tester,
  ) async {
    await tester.pumpWidget(
      _host(
        CrepusView.fromSource(
          '''
foreach items as row
  text "{row}"
''',
          data: const {
            'items': ['alpha', 'beta'],
          },
        ),
      ),
    );
    expect(find.text('alpha'), findsOneWidget);
    expect(find.text('beta'), findsOneWidget);
  });
}

/// A marker type that never exists in the tree — used to assert a disallowed
/// kind produced nothing recognizable.
class WebViewMarker extends StatelessWidget {
  const WebViewMarker({super.key});
  @override
  Widget build(BuildContext context) => const SizedBox.shrink();
}
