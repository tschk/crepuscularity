#!/usr/bin/env bun
/**
 * Generate the shared Crepuscularity Tailwind v4 default palette (hex) from theme.css.
 *
 * Usage:
 *   bun scripts/gen-tailwind-colors.mjs [--theme path] [out.rs]
 */

import { readFileSync, writeFileSync } from "fs";
import { formatHex, parse } from "culori";

const THEME_URL =
  "https://raw.githubusercontent.com/tailwindlabs/tailwindcss/v4.3.0/packages/tailwindcss/theme.css";
const DEFAULT_OUT = "crates/crepuscularity-colors/src/palette.rs";

const args = process.argv.slice(2);
let themePath = null;
let outPath = DEFAULT_OUT;
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--theme") {
    if (!args[i + 1] || args[i + 1].startsWith("-")) {
      throw new Error("--theme requires a file path");
    }
    themePath = args[++i];
  } else if (args[i].startsWith("-")) {
    throw new Error(`unknown option: ${args[i]}`);
  } else {
    outPath = args[i];
  }
}

const css = themePath
  ? readFileSync(themePath, "utf8")
  : await fetch(THEME_URL).then((r) => {
      if (!r.ok) throw new Error(`theme download failed: HTTP ${r.status}`);
      return r.text();
    });

/** @type {Array<[string, string]>} */
const entries = [];

for (const m of css.matchAll(/--color-([a-z]+)-(\d+):\s*oklch\(([^)]*)\);/g)) {
  const [, family, shade, inner] = m;
  const errorMessage = `invalid palette OKLCH color: ${family}-${shade}`;
  let hex;
  try {
    hex = formatHex(parse(`oklch(${inner})`));
  } catch (cause) {
    throw new Error(errorMessage, { cause });
  }
  if (!hex) throw new Error(errorMessage);
  entries.push([`${family}-${shade}`, hex]);
}

for (const m of css.matchAll(/--color-([a-z]+):\s*(#[0-9a-fA-F]*);/g)) {
  if (m[2].length !== 4 && m[2].length !== 7) {
    throw new Error(`unsupported palette hex color: ${m[1]} = ${m[2]}`);
  }
  entries.push([m[1], m[2].toLowerCase()]);
}

if (entries.length === 0) throw new Error("theme contains no supported colors");

entries.sort((a, b) => a[0].localeCompare(b[0]));

for (let i = 1; i < entries.length; i++) {
  if (entries[i - 1][0] === entries[i][0]) {
    throw new Error(`duplicate palette color: ${entries[i][0]}`);
  }
}

const lines = [
  "//! Full Tailwind CSS v4 default palette (OKLCH → sRGB hex).",
  "//!",
  "//! Generated from tailwindcss v4.3.0 `theme.css` via `scripts/gen-tailwind-colors.mjs`.",
  "//! Class names (`bg-blue-500`, etc.) are unchanged; resolved hex values follow v4.",
  "",
  '/// Look up a Tailwind named color like `"slate-500"` or `"red-100"`.',
  "pub fn lookup_named_color(name: &str) -> Option<&'static str> {",
  "    Some(match name {",
];
for (const [key, hex] of entries) {
  lines.push(`        "${key}" => "${hex}",`);
}
lines.push("        _ => return None,", "    })", "}", "");

writeFileSync(outPath, lines.join("\n"));
console.log(`wrote ${entries.length} colors to ${outPath}`);
