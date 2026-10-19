import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter_test/flutter_test.dart';

const _fields = [
  'margin',
  'marginHorizontal',
  'marginVertical',
  'marginTop',
  'marginBottom',
  'marginLeft',
  'marginRight',
];

void main() {
  test('margin-only styles survive decoding and wire round trips', () {
    for (final field in _fields) {
      final style = ViewStyle.fromJson({field: -2.5});
      expect(style, isNotNull, reason: field);
      expect(style!.toJson(), {field: -2.5});
      expect(ViewStyle.fromJson(style.toJson())!.toJson(), {field: -2.5});
    }
  });

  test('finite numeric strings and zero margins are retained', () {
    final style = ViewStyle.fromJson({
      'id': 'margin-probe',
      'margin': '-4.5',
      'marginHorizontal': 0,
      'marginVertical': 3,
      'marginTop': 1.25,
      'marginBottom': -2,
      'marginLeft': -3,
      'marginRight': '2.75',
    })!;
    expect(style.toJson(), {
      'id': 'margin-probe',
      'margin': -4.5,
      'marginHorizontal': 0.0,
      'marginVertical': 3.0,
      'marginTop': 1.25,
      'marginBottom': -2.0,
      'marginLeft': -3.0,
      'marginRight': 2.75,
    });
  });

  test('invalid margin scalars never enter the decoded model', () {
    for (final field in _fields) {
      for (final bad in [
        double.nan,
        double.infinity,
        double.negativeInfinity,
        'NaN',
        'Infinity',
        '-Infinity',
        '1e309',
        'bad',
        true,
        null,
      ]) {
        expect(ViewStyle.fromJson({field: bad}), isNull, reason: '$field $bad');
      }
    }
  });

  test('source margin classes retain signed values and raw tokens', () {
    const classes = [
      'm-4',
      '-mx-2',
      'my-[1.25px]',
      '-mt-px',
      'mb-[-3px]',
      'ml-[2px]',
      '-mr-[-2px]',
    ];
    final ir = viewIrFromSource('div ${classes.join(' ')}');
    expect(ir.root.single.style!.toJson(), {
      'classes': classes,
      'margin': 16.0,
      'marginHorizontal': -8.0,
      'marginVertical': 1.25,
      'marginTop': -1.0,
      'marginBottom': -3.0,
      'marginLeft': 2.0,
      'marginRight': 2.0,
    });
  });

  test('malformed margin classes preserve earlier valid hints', () {
    final ir = viewIrFromSource('div m-4 m-[bad] m-[Infinity] -mx-2 -mx-[NaN]');
    final style = ir.root.single.style!.toJson();
    expect(style['margin'], 16.0);
    expect(style['marginHorizontal'], -8.0);
  });
}
