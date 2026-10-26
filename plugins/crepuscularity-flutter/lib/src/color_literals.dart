/// Normalize CSS hex to six/eight digits, retaining RRGGBBAA alpha ordering.
String? normalizeCssHex(String value) {
  var digits = value.trim();
  if (digits.startsWith('#')) digits = digits.substring(1);
  if (!RegExp(r'^[0-9a-fA-F]+$').hasMatch(digits)) return null;
  if (digits.length == 3 || digits.length == 4) {
    digits = digits.split('').map((digit) => '$digit$digit').join();
  }
  if (digits.length != 6 && digits.length != 8) return null;
  return '#$digits';
}
