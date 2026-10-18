// Shared Crepus string escapes; unknown escapes remain literal.
String unquoteCrepusLiteral(String raw) {
  var s = raw;
  if (s.startsWith('"')) s = s.substring(1);
  if (s.endsWith('"')) s = s.substring(0, s.length - 1);
  return s.replaceAllMapped(RegExp(r'''\\([nrt"'\\])'''), (match) {
    return switch (match[1]) {
      'n' => '\n',
      'r' => '\r',
      't' => '\t',
      _ => match[1]!,
    };
  });
}
