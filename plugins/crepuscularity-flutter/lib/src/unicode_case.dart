/// Locale-independent Unicode casing matching the core DSL evaluator.
library;

part 'unicode_case_data.dart';

String _mappedRune(int rune, List<int> ranges, Map<int, String> expansions) {
  final expansion = expansions[rune];
  if (expansion != null) return expansion;
  var low = 0;
  var high = ranges.length ~/ 4 - 1;
  while (low <= high) {
    final middle = (low + high) ~/ 2;
    final offset = middle * 4;
    final start = ranges[offset];
    if (rune < start) {
      high = middle - 1;
    } else if (rune > ranges[offset + 1]) {
      low = middle + 1;
    } else {
      return String.fromCharCode(
        (rune - start) % ranges[offset + 2] == 0
            ? rune + ranges[offset + 3]
            : rune,
      );
    }
  }
  return String.fromCharCode(rune);
}

bool _inRanges(int rune, List<int> ranges) {
  var low = 0;
  var high = ranges.length ~/ 2 - 1;
  while (low <= high) {
    final middle = (low + high) ~/ 2;
    final offset = middle * 2;
    if (rune < ranges[offset]) {
      high = middle - 1;
    } else if (rune > ranges[offset + 1]) {
      low = middle + 1;
    } else {
      return true;
    }
  }
  return false;
}

String crepusUpperCase(String value) {
  final out = StringBuffer();
  for (final rune in value.runes) {
    out.write(_mappedRune(rune, _upperRanges, _upperExpansions));
  }
  return out.toString();
}

String crepusLowerCase(String value) {
  final runes = value.runes.toList(growable: false);
  final nextCased = List<bool>.filled(runes.length, false);
  var cased = false;
  for (var i = runes.length - 1; i >= 0; i--) {
    nextCased[i] = cased;
    if (!_inRanges(runes[i], _caseIgnorableRanges)) {
      cased = _inRanges(runes[i], _casedRanges);
    }
  }
  final out = StringBuffer();
  var previousCased = false;
  for (var i = 0; i < runes.length; i++) {
    final rune = runes[i];
    out.write(
      rune == 0x03A3 && previousCased && !nextCased[i]
          ? '\u03C2'
          : _mappedRune(rune, _lowerRanges, _lowerExpansions),
    );
    if (!_inRanges(rune, _caseIgnorableRanges)) {
      previousCased = _inRanges(rune, _casedRanges);
    }
  }
  return out.toString();
}
