import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter_test/flutter_test.dart';

Map<String, Object?> nested(int depth) {
  Map<String, Object?> node = {'kind': 'text', 'content': 'leaf'};
  for (var level = 1; level < depth; level++) {
    node = {
      'kind': 'stack',
      'axis': 'column',
      'children': [node],
    };
  }
  return node;
}

void main() {
  test('standalone and document decoding share the default depth boundary', () {
    final limit = CrepusLimits.defaults.maxDepth;
    final accepted = nested(limit);
    expect(ViewNode.fromJson(accepted), isNot(isA<UnsupportedNode>()));
    expect(
      ViewIr.fromJson({
        'version': 7,
        'root': [accepted],
      }).root,
      hasLength(1),
    );

    final oversized = nested(limit + 1);
    expect(ViewNode.fromJson(oversized), isA<UnsupportedNode>());
    expect(
      ViewIr.fromJson({
        'version': 7,
        'root': [oversized],
      }).root,
      isEmpty,
    );
  });
}
