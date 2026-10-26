import 'dart:async';
import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

Widget _host(Widget child) => MaterialApp(home: Scaffold(body: child));

class _DelayedTimer implements Timer {
  _DelayedTimer(this.callback);
  final void Function(Timer) callback;
  @override
  bool isActive = true;
  @override
  int tick = 0;
  void fire(int elapsedTicks) {
    if (!isActive) return;
    tick = elapsedTicks;
    callback(this);
  }

  @override
  void cancel() => isActive = false;
}

void main() {
  group('duration parsing', () {
    test('overflowing durations become dead timers', () {
      expect(parseCrepusDuration('9' * 400), Duration.zero);
      expect(parseCrepusDuration('${'9' * 300}h'), Duration.zero);
      expect(
        parseCrepusDuration('999999999999999999999999999999s'),
        Duration.zero,
      );
    });

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
    testWidgets('short active intervals accumulate across pauses', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(CrepusView.fromSource('timer duration=10s countup')),
      );
      for (var cycle = 0; cycle < 10; cycle++) {
        await tester.tap(find.text('Start'));
        await tester.pump(const Duration(milliseconds: 150));
        await tester.tap(find.text('Pause'));
        await tester.pump();
      }
      expect(find.text('00:01'), findsOneWidget);
      await tester.pump(const Duration(seconds: 5));
      expect(find.text('00:01'), findsOneWidget);
      await tester.tap(find.text('Start'));
      await tester.pump(const Duration(milliseconds: 500));
      await tester.tap(find.text('Pause'));
      await tester.pump();
      expect(find.text('00:02'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('delayed ticks catch up and paused time stays excluded', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(CrepusView.fromSource('timer duration=10s')),
      );
      late _DelayedTimer ticker;
      Future<void> start() => runZoned(
        () => tester.tap(find.text('Start')),
        zoneSpecification: ZoneSpecification(
          createPeriodicTimer: (self, parent, zone, duration, callback) {
            expect(duration, const Duration(milliseconds: 200));
            return ticker = _DelayedTimer(callback);
          },
        ),
      );
      await start();
      await tester.pump(const Duration(seconds: 5));
      ticker.fire(25);
      await tester.pump();
      expect(find.text('00:05'), findsOneWidget);
      await tester.tap(find.text('Pause'));
      await tester.pump(const Duration(seconds: 30));
      expect(ticker.isActive, isFalse);
      expect(find.text('00:05'), findsOneWidget);
      await start();
      await tester.pump(const Duration(seconds: 1));
      ticker.fire(5);
      await tester.pump();
      expect(find.text('00:04'), findsOneWidget);
      await tester.pump(const Duration(seconds: 5));
      ticker.fire(30);
      await tester.pump();
      expect(find.text('Done'), findsOneWidget);
      expect(find.text('00:00'), findsOneWidget);
      expect(ticker.isActive, isFalse);
      await tester.tap(find.text('Reset'));
      await tester.pump();
      expect(find.text('00:10'), findsOneWidget);
      expect(tester.takeException(), isNull);
    });

    testWidgets('narrow timer controls remain usable through completion', (
      tester,
    ) async {
      for (final (width, scale) in [
        (64.0, 1.0),
        (96.0, 1.0),
        (180.0, 1.0),
        (180.0, 3.0),
      ]) {
        await tester.pumpWidget(
          _host(
            MediaQuery(
              data: MediaQueryData(textScaler: TextScaler.linear(scale)),
              child: Align(
                alignment: Alignment.topLeft,
                child: SizedBox(
                  width: width,
                  child: CrepusView.fromSource('timer duration=60s'),
                ),
              ),
            ),
          ),
        );
        expect(tester.takeException(), isNull, reason: 'width=$width');
        await tester.tap(find.widgetWithText(TextButton, 'Start'));
        await tester.pump();
        await tester.pump(const Duration(seconds: 2));
        expect(find.text('00:58'), findsOneWidget);
        await tester.tap(find.widgetWithText(TextButton, 'Pause'));
        await tester.pump();
        await tester.tap(find.widgetWithText(TextButton, 'Reset'));
        await tester.pump();
        expect(find.text('01:00'), findsOneWidget);
        await tester.tap(find.widgetWithText(TextButton, 'Start'));
        await tester.pump();
        await tester.pump(const Duration(seconds: 60));
        expect(find.text('Done'), findsOneWidget);
        expect(tester.takeException(), isNull);
        await tester.tap(find.widgetWithText(TextButton, 'Reset'));
        await tester.pump();
      }
    });

    testWidgets('enabling reduced motion stops a running timer', (
      tester,
    ) async {
      Widget host(bool disabled) => MaterialApp(
        home: MediaQuery(
          data: MediaQueryData(disableAnimations: disabled),
          child: Scaffold(
            body: CrepusView.fromSource('timer duration=60s autostart'),
          ),
        ),
      );
      await tester.pumpWidget(host(false));
      await tester.pump(const Duration(seconds: 10));
      expect(find.text('00:50'), findsOneWidget);
      await tester.pumpWidget(host(true));
      await tester.pump(const Duration(seconds: 10));
      expect(find.text('00:50'), findsOneWidget);
      expect(find.text('Start'), findsOneWidget);
    });

    testWidgets('Start immediately switches the control to Pause', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(CrepusView.fromSource('timer duration=60s')),
      );
      await tester.tap(find.text('Start'));
      await tester.pump();
      expect(find.text('Pause'), findsOneWidget);
      expect(find.text('Start'), findsNothing);
      await tester.tap(find.text('Pause'));
      await tester.pump();
    });

    testWidgets('ordinary rebuild keeps elapsed time and running state', (
      tester,
    ) async {
      await tester.pumpWidget(
        _host(CrepusView.fromSource('timer "Before" duration=60s autostart')),
      );
      await tester.pump(const Duration(seconds: 10));
      expect(find.text('00:50'), findsOneWidget);
      await tester.pumpWidget(
        _host(
          CrepusView.fromSource('timer "After" duration=60s countup autostart'),
        ),
      );
      expect(find.text('After'), findsOneWidget);
      expect(find.text('00:10'), findsOneWidget);
      expect(find.text('Pause'), findsOneWidget);
      await tester.pump(const Duration(seconds: 5));
      expect(find.text('00:15'), findsOneWidget);
      await tester.tap(find.text('Pause'));
      await tester.pump();
      expect(tester.takeException(), isNull);
    });

    testWidgets('replacement duration honors autostart', (tester) async {
      await tester.pumpWidget(
        _host(CrepusView.fromSource('timer duration=60s autostart')),
      );
      await tester.pump(const Duration(seconds: 10));
      await tester.pumpWidget(
        _host(CrepusView.fromSource('timer duration=120s autostart')),
      );
      await tester.pump(const Duration(seconds: 10));
      expect(find.text('01:50'), findsOneWidget);
      expect(find.text('Pause'), findsOneWidget);
      await tester.tap(find.text('Pause'));
      await tester.pump();
    });

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
