import { expect, test } from "bun:test";

test("isSafeUrl testing", () => {
  const isSafeUrl = (value) => {
    return value.startsWith("http://") ||
           value.startsWith("https://") ||
           value.startsWith("mailto:") ||
           value.startsWith("#") ||
           value.startsWith("/") && !value.startsWith("//") && !value.startsWith("/\\");
  };

  expect(isSafeUrl("http://example.com")).toBe(true);
  expect(isSafeUrl("https://example.com")).toBe(true);
  expect(isSafeUrl("mailto:test@example.com")).toBe(true);
  expect(isSafeUrl("#anchor")).toBe(true);
  expect(isSafeUrl("/path/to/resource")).toBe(true);

  expect(isSafeUrl("javascript:alert(1)")).toBe(false);
  expect(isSafeUrl("javascript://alert(1)")).toBe(false);
  expect(isSafeUrl("//example.com")).toBe(false);
  expect(isSafeUrl("/\\example.com")).toBe(false);
  expect(isSafeUrl("\\\\example.com")).toBe(false);
  expect(isSafeUrl("data:text/html,<html>")).toBe(false);
  expect(isSafeUrl("vbscript:msgbox(1)")).toBe(false);
});
