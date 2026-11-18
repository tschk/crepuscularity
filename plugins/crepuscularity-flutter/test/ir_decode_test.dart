import 'dart:convert';

import 'package:crepuscularity_flutter/crepuscularity_flutter.dart';
import 'package:flutter_test/flutter_test.dart';

/// Decoder-level tests for `ViewIr.fromJson` / `ViewNode.fromJson`. The IR
/// arrives from a language model, so every case here is written as untrusted
/// input: the contract is "degrade to something inert", never "throw".
void main() {
  group('document envelope', () {
    test('empty document decodes to an empty root', () {
      final ir = ViewIr.fromJson(const {});
      expect(ir.version, 0);
      expect(ir.root, isEmpty);
    });

    test('missing version defaults to 0 without throwing', () {
      final ir = ViewIr.fromJson(const {
        'root': [
          {'kind': 'text', 'content': 'a'},
        ],
      });
      expect(ir.version, 0);
      expect(ir.root, hasLength(1));
    });

    test('version is coerced from strings and floats, never rejected', () {
      expect(ViewIr.fromJson(const {'version': '5'}).version, 5);
      expect(ViewIr.fromJson(const {'version': 4.7}).version, 5);
      expect(ViewIr.fromJson(const {'version': 'five'}).version, 0);
      expect(ViewIr.fromJson(const {'version': null}).version, 0);
      expect(ViewIr.fromJson(const {'version': true}).version, 0);
    });

    test('a wrong version still decodes — the host owns version policy', () {
      final ir = ViewIr.fromJson(const {
        'version': 999,
        'root': [
          {'kind': 'text', 'content': 'a'},
        ],
      });
      expect(ir.version, 999);
      expect(ir.root.single, isA<TextNode>());
    });

    test('a non-list root yields an empty document', () {
      for (final root in const [
        'text',
        42,
        true,
        {'kind': 'text'},
      ]) {
        expect(ViewIr.fromJson({'version': 5, 'root': root}).root, isEmpty);
      }
      expect(ViewIr.fromJson(const {'version': 5, 'root': null}).root, isEmpty);
    });

    test('an empty root list is a valid empty document', () {
      final ir = ViewIr.fromJson(const {'version': 5, 'root': []});
      expect(ir.root, isEmpty);
    });

    test('root entries that are not objects become UnsupportedNode', () {
      final ir = ViewIr.fromJson(const {
        'version': 5,
        'root': [null, 3, 'text', true, []],
      });
      expect(ir.root, hasLength(5));
      expect(ir.root.whereType<UnsupportedNode>(), hasLength(5));
      expect(ir.root.whereType<UnsupportedNode>().map((n) => n.kind).toSet(), {
        '<non-object>',
      });
    });

    test('decoding survives a JSON round trip of a real document', () {
      final decoded =
          jsonDecode(
                '{"version":5,"root":[{"kind":"text","content":"hi"},'
                '{"kind":"stack","axis":"row","children":[]}]}',
              )
              as Map<String, Object?>;
      final ir = ViewIr.fromJson(decoded);
      expect(ir.version, 5);
      expect(ir.root, hasLength(2));
      expect((ir.root[0] as TextNode).content, 'hi');
      expect((ir.root[1] as StackNode).axis, StackAxis.row);
    });
  });

  group('unknown and hostile kinds', () {
    test('every kind outside the allowlist decodes to UnsupportedNode', () {
      const hostile = [
        'webView',
        'webview',
        'iframe',
        'filePicker',
        'dropzone',
        'input',
        'picker',
        'slider',
        'tabs',
        'slot',
        'embed',
        'script',
        'eval',
        '__proto__',
        'Text',
        'TEXT',
        ' text',
        'text ',
        '',
      ];
      for (final kind in hostile) {
        final node = ViewNode.fromJson({'kind': kind});
        expect(node, isA<UnsupportedNode>(), reason: 'kind=$kind');
        expect((node as UnsupportedNode).kind, kind);
      }
    });

    test('the allowlist itself is exactly the kinds the decoder accepts', () {
      for (final kind in kAllowedKinds) {
        expect(
          ViewNode.fromJson({'kind': kind}),
          isNot(isA<UnsupportedNode>()),
          reason: 'kind=$kind should decode',
        );
      }
      expect(kAllowedKinds, hasLength(19));
    });

    test('a missing or non-string kind is UnsupportedNode', () {
      expect(
        (ViewNode.fromJson(const {}) as UnsupportedNode).kind,
        '<no-kind>',
      );
      expect(
        (ViewNode.fromJson(const {'kind': 7}) as UnsupportedNode).kind,
        '<no-kind>',
      );
      expect(
        (ViewNode.fromJson(const {'kind': null}) as UnsupportedNode).kind,
        '<no-kind>',
      );
    });

    test('an unsupported kind carrying children does not leak them', () {
      final node = ViewNode.fromJson(const {
        'kind': 'webView',
        'children': [
          {'kind': 'text', 'content': 'smuggled'},
        ],
      });
      expect(node, isA<UnsupportedNode>());
      expect(childrenOf(node), isEmpty);
    });
  });

  group('wrong value types on known properties', () {
    test('text coerces a non-string content to empty', () {
      for (final content in const [42, true, null, [], {}]) {
        final node = ViewNode.fromJson({'kind': 'text', 'content': content});
        expect((node as TextNode).content, '');
      }
    });

    test('text bind must be a string', () {
      expect(
        (ViewNode.fromJson(const {'kind': 'text', 'bind': 3}) as TextNode).bind,
        isNull,
      );
      expect(
        (ViewNode.fromJson(const {'kind': 'text', 'bind': 'a.b'}) as TextNode)
            .bind,
        'a.b',
      );
    });

    test('a non-list children collapses to an empty child list', () {
      for (final children in const ['a', 3, true, {}, null]) {
        final node = ViewNode.fromJson({'kind': 'stack', 'children': children});
        expect((node as StackNode).children, isEmpty);
      }
    });

    test('axis falls back to column for anything but the literal "row"', () {
      for (final axis in const [null, 'ROW', 'Row', 'horizontal', 3, true]) {
        final node = ViewNode.fromJson({'kind': 'stack', 'axis': axis});
        expect((node as StackNode).axis, StackAxis.column, reason: '$axis');
      }
      expect(
        (ViewNode.fromJson(const {'kind': 'stack', 'axis': 'row'}) as StackNode)
            .axis,
        StackAxis.row,
      );
    });

    test('numeric properties accept numeric strings and reject junk', () {
      final fromString = ViewNode.fromJson(const {
        'kind': 'progress',
        'value': '40',
        'max': '80',
      });
      expect((fromString as ProgressNode).value, 40);
      expect(fromString.max, 80);

      final junk = ViewNode.fromJson(const {
        'kind': 'progress',
        'value': 'forty',
        'max': true,
      });
      expect((junk as ProgressNode).value, 0);
      expect(junk.max, 100);
    });

    test('non-finite numbers never reach the model', () {
      final node = ViewNode.fromJson(const {
        'kind': 'meter',
        'value': 'nan',
        'min': '-inf',
        'max': 'infinity',
      });
      expect((node as MeterNode).value, 0);
      expect(node.min, 0);
      expect(node.max, 100);
      expect(node.value.isFinite, isTrue);
    });

    test('checked / ordered are strictly true, not truthy', () {
      for (final value in const ['true', 1, 'yes', {}, null]) {
        expect(
          (ViewNode.fromJson({'kind': 'toggle', 'checked': value})
                  as ToggleNode)
              .checked,
          isFalse,
          reason: '$value',
        );
      }
      expect(
        (ViewNode.fromJson(const {'kind': 'toggle', 'checked': true})
                as ToggleNode)
            .checked,
        isTrue,
      );
      expect(
        (ViewNode.fromJson(const {'kind': 'list', 'ordered': 'true'})
                as ListNode)
            .ordered,
        isFalse,
      );
    });

    test('action strings are surfaced verbatim, never interpreted', () {
      const payloads = [
        'prompt:Draft the email',
        'javascript:alert(1)',
        '../../etc/passwd',
        r'${jndi:ldap://x}',
        '<script>alert(1)</script>',
      ];
      for (final payload in payloads) {
        final node =
            ViewNode.fromJson({'kind': 'button', 'onClick': payload})
                as ButtonNode;
        expect(node.onClick, payload);
      }
    });

    test('a non-string action is dropped rather than stringified', () {
      final node =
          ViewNode.fromJson(const {'kind': 'button', 'onClick': 42})
              as ButtonNode;
      expect(node.onClick, isNull);
    });

    test('forEach defaults itemName to "item" and bind to empty', () {
      final node = ViewNode.fromJson(const {'kind': 'forEach'}) as ForEachNode;
      expect(node.bind, '');
      expect(node.itemName, 'item');
      expect(node.itemBody, isEmpty);
    });

    test('if distinguishes an absent else from an empty else', () {
      final absent = ViewNode.fromJson(const {'kind': 'if'}) as IfNode;
      expect(absent.condition, '');
      expect(absent.elseChildren, isNull);
      final empty =
          ViewNode.fromJson(const {'kind': 'if', 'elseChildren': []}) as IfNode;
      expect(empty.elseChildren, isEmpty);
    });

    test('every leaf kind decodes with no properties at all', () {
      for (final kind in kAllowedKinds) {
        expect(
          () => ViewNode.fromJson({'kind': kind}),
          returnsNormally,
          reason: kind,
        );
      }
    });
  });

  group('style decoding', () {
    test('a non-map style is ignored', () {
      for (final style in const ['p-4', 3, true, [], null]) {
        expect(ViewStyle.fromJson(style), isNull, reason: '$style');
      }
    });

    test('an all-junk style map collapses to null', () {
      expect(
        ViewStyle.fromJson(const {
          'padding': 'lots',
          'fontSize': true,
          'foregroundColor': 9,
          'italic': 'yes',
          'unknownProperty': 'whatever',
        }),
        isNull,
      );
    });

    test('a style with one recognized property survives', () {
      final style = ViewStyle.fromJson(const {
        'padding': '12',
        'unknownProperty': 'ignored',
      });
      expect(style, isNotNull);
      expect(style!.padding, 12);
      expect(style.fontSize, isNull);
    });

    test('fontWeight rounds from a double and rejects a string name', () {
      expect(ViewStyle.fromJson(const {'fontWeight': 649.6})!.fontWeight, 650);
      expect(ViewStyle.fromJson(const {'fontWeight': 'bold'}), isNull);
    });

    test('booleans are tri-state: true, false, or absent', () {
      expect(ViewStyle.fromJson(const {'italic': false})!.italic, isFalse);
      expect(ViewStyle.fromJson(const {'italic': true})!.italic, isTrue);
      expect(ViewStyle.fromJson(const {'italic': 1}), isNull);
    });

    test('every style field round trips through the decoder', () {
      final style = ViewStyle.fromJson(const {
        'padding': 1,
        'paddingHorizontal': 2,
        'paddingVertical': 3,
        'paddingTop': 4,
        'paddingBottom': 5,
        'paddingLeft': 6,
        'paddingRight': 7,
        'fontSize': 8,
        'fontWeight': 700,
        'textAlign': 'center',
        'foregroundColor': '#111111',
        'backgroundColor': '#222222',
        'cornerRadius': 9,
        'borderWidth': 10,
        'borderColor': '#333333',
        'opacity': 0.5,
        'italic': true,
        'underline': true,
        'strikethrough': true,
      })!;
      expect(style.isEmpty, isFalse);
      expect(style.paddingHorizontal, 2);
      expect(style.paddingRight, 7);
      expect(style.textAlign, 'center');
      expect(style.borderColor, '#333333');
      expect(style.opacity, 0.5);
      expect(style.strikethrough, isTrue);
    });

    test('a style on an unsupported node is discarded with the node', () {
      final node = ViewNode.fromJson(const {
        'kind': 'webView',
        'style': {'padding': 8},
      });
      expect(node.style, isNull);
    });
  });

  group('limits', () {
    test('a document exactly at the node cap is kept', () {
      final root = List.generate(60, (i) => {'kind': 'text', 'content': '$i'});
      expect(ViewIr.fromJson({'version': 5, 'root': root}).root, hasLength(60));
    });

    test('one node past the cap empties the document', () {
      final root = List.generate(61, (i) => {'kind': 'text', 'content': '$i'});
      expect(ViewIr.fromJson({'version': 5, 'root': root}).root, isEmpty);
    });

    test('nested nodes count toward the same node cap', () {
      Map<String, Object?> nest(int n) => n == 0
          ? {'kind': 'text', 'content': 'leaf'}
          : {
              'kind': 'stack',
              'children': [nest(n - 1)],
            };
      // 7 nodes deep is inside both caps; 61 wide siblings under one stack is
      // inside the depth cap but past the node cap.
      expect(
        ViewIr.fromJson({
          'version': 5,
          'root': [nest(6)],
        }).root,
        hasLength(1),
      );
      expect(
        ViewIr.fromJson({
          'version': 5,
          'root': [
            {
              'kind': 'stack',
              'children': List.generate(
                61,
                (i) => {'kind': 'text', 'content': '$i'},
              ),
            },
          ],
        }).root,
        isEmpty,
      );
    });

    test('a deeply nested document is rejected, not overflowed', () {
      Map<String, Object?> nest(int n) => n == 0
          ? {'kind': 'text', 'content': 'bottom'}
          : {
              'kind': 'stack',
              'children': [nest(n - 1)],
            };
      final ir = ViewIr.fromJson({
        'version': 5,
        'root': [nest(40)],
      });
      expect(ir.root, isEmpty);
    });

    test('decoding a pathologically deep document does not throw', () {
      Map<String, Object?> nest(int n) => n == 0
          ? {'kind': 'text', 'content': 'bottom'}
          : {
              'kind': 'stack',
              'children': [nest(n - 1)],
            };
      expect(
        () => ViewIr.fromJson({
          'version': 5,
          'root': [nest(2000)],
        }),
        returnsNormally,
      );
    });

    test('an enormous flat document is rejected quickly', () {
      final root = List.generate(
        20000,
        (i) => {'kind': 'text', 'content': '$i'},
      );
      final ir = ViewIr.fromJson({'version': 5, 'root': root});
      expect(ir.root, isEmpty);
    });

    test('custom limits widen and narrow the caps', () {
      final root = List.generate(100, (i) => {'kind': 'text', 'content': '$i'});
      expect(
        ViewIr.fromJson({
          'version': 5,
          'root': root,
        }, limits: const CrepusLimits(maxNodes: 200)).root,
        hasLength(100),
      );
      expect(
        ViewIr.fromJson({
          'version': 5,
          'root': [
            {'kind': 'text', 'content': 'a'},
            {'kind': 'text', 'content': 'b'},
          ],
        }, limits: const CrepusLimits(maxNodes: 1)).root,
        isEmpty,
      );
    });

    test('if branches both count toward the caps', () {
      final ir = ViewIr.fromJson({
        'version': 5,
        'root': [
          {
            'kind': 'if',
            'condition': 'x',
            'thenChildren': List.generate(
              30,
              (i) => {'kind': 'text', 'content': '$i'},
            ),
            'elseChildren': List.generate(
              31,
              (i) => {'kind': 'text', 'content': '$i'},
            ),
          },
        ],
      });
      expect(ir.root, isEmpty);
    });
  });

  group('childrenOf', () {
    test('reports children for every container kind', () {
      Map<String, Object?> leaf() => {'kind': 'text', 'content': 'x'};
      final cases = <String, Map<String, Object?>>{
        'stack': {
          'kind': 'stack',
          'children': [leaf()],
        },
        'scroll': {
          'kind': 'scroll',
          'children': [leaf()],
        },
        'list': {
          'kind': 'list',
          'children': [leaf()],
        },
        'listItem': {
          'kind': 'listItem',
          'children': [leaf()],
        },
        'link': {
          'kind': 'link',
          'href': 'https://example.com',
          'children': [leaf()],
        },
        'forEach': {
          'kind': 'forEach',
          'itemBody': [leaf()],
        },
        'if': {
          'kind': 'if',
          'thenChildren': [leaf()],
        },
      };
      cases.forEach((name, json) {
        expect(childrenOf(ViewNode.fromJson(json)), hasLength(1), reason: name);
      });
    });

    test('if reports then and else children together', () {
      final node = ViewNode.fromJson(const {
        'kind': 'if',
        'thenChildren': [
          {'kind': 'text', 'content': 'a'},
        ],
        'elseChildren': [
          {'kind': 'text', 'content': 'b'},
          {'kind': 'text', 'content': 'c'},
        ],
      });
      expect(childrenOf(node), hasLength(3));
    });

    test('every leaf kind reports no children', () {
      const leaves = [
        'text',
        'button',
        'toggle',
        'checkbox',
        'progress',
        'timer',
        'meter',
        'badge',
        'divider',
        'spacer',
        'image',
      ];
      for (final kind in leaves) {
        expect(childrenOf(ViewNode.fromJson({'kind': kind})), isEmpty);
      }
      expect(childrenOf(const UnsupportedNode('webView')), isEmpty);
    });
  });

  group('link', () {
    test('decodes its href, target, rel and children', () {
      final node =
          ViewNode.fromJson(const {
                'kind': 'link',
                'href': 'https://example.com/a',
                'target': '_blank',
                'rel': 'noopener',
                'children': [
                  {'kind': 'text', 'content': 'a'},
                  {'kind': 'text', 'content': 'b'},
                ],
              })
              as LinkNode;
      expect(node.href, 'https://example.com/a');
      expect(node.target, '_blank');
      expect(node.rel, 'noopener');
      expect(node.children, hasLength(2));
    });

    test('missing or non-string fields degrade instead of throwing', () {
      final cases = <String, Map<String, Object?>>{
        'no href': {'kind': 'link'},
        'numeric href': {'kind': 'link', 'href': 7},
        'children not a list': {
          'kind': 'link',
          'href': 'x',
          'children': 'nope',
        },
      };
      cases.forEach((name, json) {
        final node = ViewNode.fromJson(json) as LinkNode;
        expect(node.target, isNull, reason: name);
        expect(node.rel, isNull, reason: name);
        expect(node.children, isEmpty, reason: name);
      });
      expect((ViewNode.fromJson(const {'kind': 'link'}) as LinkNode).href, '');
    });

    test('a link nests under the depth cap like any other container', () {
      final ir = ViewIr.fromJson(const {
        'version': 7,
        'root': [
          {
            'kind': 'link',
            'href': 'https://example.com',
            'children': [
              {'kind': 'text', 'content': 'deep'},
            ],
          },
        ],
      });
      expect(childrenOf(ir.root.single), hasLength(1));
    });
  });

  group('style identity fields', () {
    test('id and classes decode verbatim', () {
      final style = ViewStyle.fromJson(const {
        'id': 'hero',
        'classes': ['p-2', 'text-lg', 'not-a-class'],
      })!;
      expect(style.id, 'hero');
      expect(style.classes, ['p-2', 'text-lg', 'not-a-class']);
    });

    test('an id alone is enough to keep the style', () {
      expect(ViewStyle.fromJson(const {'id': 'solo'})!.id, 'solo');
      expect(
        ViewStyle.fromJson(const {
          'classes': ['solo'],
        })!.classes,
        ['solo'],
      );
      expect(ViewStyle.fromJson(const <String, Object?>{}), isNull);
    });

    test('non-string id and non-string class entries are dropped', () {
      final cases = <String, Map<String, Object?>>{
        'numeric id': {'id': 7, 'classes': <Object?>[]},
        'list id': {
          'id': ['a'],
          'classes': <Object?>[],
        },
        'classes not a list': {'classes': 'p-2'},
      };
      cases.forEach((name, json) {
        final style = ViewStyle.fromJson({...json, 'padding': 4})!;
        expect(style.id, isNull, reason: name);
        expect(style.classes, isEmpty, reason: name);
      });
      final mixed = ViewStyle.fromJson(const {
        'classes': ['ok', 3, null, 'fine'],
      })!;
      expect(mixed.classes, ['ok', 'fine']);
    });

    test('id and classes survive a toJson/fromJson round trip', () {
      const original = ViewStyle(
        id: 'hero',
        classes: ['p-2', 'text-lg'],
        padding: 8,
        fontSize: 17,
        foregroundColor: '#ef4444',
      );
      final json = original.toJson();
      expect(json['id'], 'hero');
      expect(json['classes'], ['p-2', 'text-lg']);
      final decoded = ViewStyle.fromJson(json)!;
      expect(decoded.id, 'hero');
      expect(decoded.classes, ['p-2', 'text-lg']);
      expect(decoded.padding, 8);
      expect(decoded.fontSize, 17);
      expect(decoded.foregroundColor, '#ef4444');
      expect(decoded.toJson(), json);
    });

    test('absent fields are omitted rather than written as null', () {
      final json = const ViewStyle(padding: 4).toJson();
      expect(json.containsKey('id'), isFalse);
      expect(json.containsKey('classes'), isFalse);
      expect(json.containsKey('fontSize'), isFalse);
      expect(json, const {'padding': 4.0});
    });
  });
}
