import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Widget _host(Widget child) => MaterialApp(home: Scaffold(body: child));

void main() {
  group('duration parsing', () {
    test('reads the units an author would write', () {
      expect(parseCrepusDuration('25m'), const Duration(minutes: 25));
      expect(parseCrepusDuration('90s'), const Duration(seconds: 90));
      expect(parseCrepusDuration('1h'), const Duration(hours: 1));
      expect(parseCrepusDuration('500ms'), const Duration(milliseconds: 500));
      expect(parseCrepusDuration('45'), const Duration(seconds: 45));
      expect(parseCrepusDuration('1.5m'), const Duration(seconds: 90));
    });

    test('a malformed duration is a dead timer, not a thrown message', () {
      for (final bad in ['', '  ', 'soon', '-5m', '0', 'NaN', '5 years']) {
        expect(parseCrepusDuration(bad), Duration.zero, reason: bad);
      }
      expect(parseCrepusDuration(null), Duration.zero);
    });
  });

  group('timer node', () {
    test('is inside the audited allowlist', () {
      expect(kAllowedKinds, contains('timer'));
    });

    test('decodes from View IR', () {
      final ir = ViewIr.fromJson({
        'root': [
          {
            'kind': 'timer',
            'label': 'Pomodoro',
            'duration': '25m',
            'autostart': true,
          },
        ],
      });
      final node = ir.root.single as TimerNode;
      expect(node.label, 'Pomodoro');
      expect(node.duration, const Duration(minutes: 25));
      expect(node.autostart, isTrue);
    });

    test('parses from .crepus source', () {
      final ir = viewIrFromSource('timer "Focus" duration=90s autostart');
      final node = ir.root.single as TimerNode;
      expect(node.label, 'Focus');
      expect(node.duration, const Duration(seconds: 90));
      expect(node.autostart, isTrue);
    });

    testWidgets('actually counts down, which progress never did', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(CrepusView.fromSource('timer "Focus" duration=60s autostart')),
      );
      await tester.pump();
      expect(find.text('01:00'), findsOneWidget);

      await tester.pump(const Duration(seconds: 10));
      expect(find.text('00:50'), findsOneWidget);

      // Stop the ticker so the test can settle.
      await tester.tap(find.text('Pause'));
      await tester.pump();
      expect(find.text('Start'), findsOneWidget);
    });

    testWidgets('reset returns it to the top', (tester) async {
      await tester.pumpWidget(
        _host(CrepusView.fromSource('timer "Focus" duration=60s autostart')),
      );
      await tester.pump(const Duration(seconds: 20));
      expect(find.text('00:40'), findsOneWidget);
      await tester.tap(find.text('Reset'));
      await tester.pump();
      expect(find.text('01:00'), findsOneWidget);
      expect(find.text('Start'), findsOneWidget);
    });

    testWidgets('holds still under reduce motion, so pumpAndSettle returns', (
      tester,
    ) async {
      await tester.pumpWidget(
        MaterialApp(
          home: MediaQuery(
            data: const MediaQueryData(disableAnimations: true),
            child: Scaffold(
              body: CrepusView.fromSource(
                'timer "Focus" duration=60s autostart',
              ),
            ),
          ),
        ),
      );
      await tester.pumpAndSettle();
      expect(find.text('01:00'), findsOneWidget);
    });

    testWidgets('counts up when asked', (tester) async {
      await tester.pumpWidget(
        _host(
          CrepusView.fromSource(
            'timer "Elapsed" duration=60s autostart countup',
          ),
        ),
      );
      await tester.pump();
      expect(find.text('00:00'), findsOneWidget);
      await tester.pump(const Duration(seconds: 15));
      expect(find.text('00:15'), findsOneWidget);
      await tester.tap(find.text('Pause'));
      await tester.pump();
    });
  });
}
