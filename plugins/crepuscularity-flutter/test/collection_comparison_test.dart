import 'package:flutter_test/flutter_test.dart';

import 'null_comparison_test.dart' show expectCondition;

void main() {
  testWidgets(
    'collections compare structurally rather than by display spelling',
    (tester) async {
      for (final (left, right, equal) in <(Object?, Object?, bool)>[
        ([], '[]', false),
        (1.0, '1', true),
        (1.0, '1.0', false),
        (-0.0, '-0', true),
        (1, '01', false),
        ({}, '{}', false),
        ([], {}, false),
        ([1, 2], [1, 2], true),
        ([1, 2], [2, 1], false),
        ([1], [1, 2], false),
        ({'a': 1, 'b': 2}, {'b': 2, 'a': 1}, true),
        ({'a': 1}, {'b': 1}, false),
        ({'a': null}, {'a': ''}, true),
        (
          {
            'a': [1, true, null],
          },
          {
            'a': ['1', 'true', ''],
          },
          true,
        ),
        (
          {
            'a': [1],
          },
          {'a': '[1]'},
          false,
        ),
        (
          [
            {'x': 1},
          ],
          [
            {'x': 2},
          ],
          false,
        ),
        (
          [
            ['a, b'],
          ],
          [
            ['a', 'b'],
          ],
          false,
        ),
      ]) {
        for (final condition in ['left == right', 'right == left']) {
          await expectCondition(
            tester,
            condition,
            equal,
            data: {'left': left, 'right': right},
          );
        }
        await expectCondition(
          tester,
          'left != right',
          !equal,
          data: {'left': left, 'right': right},
        );
      }
    },
  );
  testWidgets('shared and cyclic host collections terminate safely', (
    tester,
  ) async {
    final left = <String, Object?>{'value': 1};
    final right = <String, Object?>{'value': '1'};
    left['self'] = left;
    right['self'] = right;
    await expectCondition(
      tester,
      'left == right',
      true,
      data: {'left': left, 'right': right},
    );
    right['value'] = 2;
    await expectCondition(
      tester,
      'left == right',
      false,
      data: {'left': left, 'right': right},
    );
    final shared = {'value': 1};
    await expectCondition(
      tester,
      'left == right',
      true,
      data: {
        'left': [shared, shared],
        'right': [
          {'value': 1},
          {'value': 1},
        ],
      },
    );
  });
}
