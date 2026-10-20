import 'dart:collection';

import 'package:flutter_test/flutter_test.dart';

import 'null_comparison_test.dart' show expectCondition;

class CountedList extends ListBase<Object?> {
  CountedList(int count, Object? value) : backing = List.filled(count, value);
  final List<Object?> backing;
  int reads = 0;
  @override
  int get length => backing.length;
  @override
  set length(int value) => backing.length = value;
  @override
  Object? operator [](int index) {
    reads++;
    return backing[index];
  }

  @override
  void operator []=(int index, Object? value) => backing[index] = value;
}

class CountedMap extends MapBase<int, Object?> {
  CountedMap(int count, Object? value)
    : backing = {for (var index = 0; index < count; index++) index: value};
  final Map<int, Object?> backing;
  int reads = 0;
  @override
  Iterable<int> get keys => backing.keys;
  @override
  bool containsKey(Object? key) => backing.containsKey(key);
  @override
  Object? operator [](Object? key) {
    reads++;
    return backing[key];
  }

  @override
  void operator []=(int key, Object? value) => backing[key] = value;
  @override
  void clear() => backing.clear();
  @override
  Object? remove(Object? key) => backing.remove(key);
}

void main() {
  testWidgets('wide unequal lists stop reading after the first compared pair', (
    tester,
  ) async {
    final left = CountedList(10000, 0);
    final right = CountedList(10000, 1);
    await expectCondition(
      tester,
      'left == right',
      false,
      data: {'left': left, 'right': right},
    );
    expect((left.reads, right.reads), (1, 1));
  });
  testWidgets('wide unequal maps stop reading after the first compared pair', (
    tester,
  ) async {
    final left = CountedMap(10000, 0);
    final right = CountedMap(10000, 1);
    await expectCondition(
      tester,
      'left == right',
      false,
      data: {'left': left, 'right': right},
    );
    expect((left.reads, right.reads), (1, 1));
  });
  testWidgets('equal collections still compare every value', (tester) async {
    final lists = (CountedList(1000, 1), CountedList(1000, '1'));
    await expectCondition(
      tester,
      'left == right',
      true,
      data: {'left': lists.$1, 'right': lists.$2},
    );
    expect((lists.$1.reads, lists.$2.reads), (1000, 1000));
    final maps = (CountedMap(1000, 1), CountedMap(1000, '1'));
    await expectCondition(
      tester,
      'left == right',
      true,
      data: {'left': maps.$1, 'right': maps.$2},
    );
    expect((maps.$1.reads, maps.$2.reads), (1000, 1000));
  });
  testWidgets('deep comparison uses iterator frames without recursion', (
    tester,
  ) async {
    Object? left = 1;
    Object? right = '1';
    for (var depth = 0; depth < 4096; depth++) {
      left = [left];
      right = [right];
    }
    await expectCondition(
      tester,
      'left == right',
      true,
      data: {'left': left, 'right': right},
    );
  });
}
