# Balanced Probe Summary

Generated: 2026-06-25T05:45:00Z
Phase: L4 (Lite Probe)
Target: `piolium` balanced audit of `softwarecorporation/crepuscularity` @ `main`

---

## Status

**Complete** — 10 findings drafted, covering both attack surface slices.

## Attack Surface Slices Selected

| # | Slice | Priority | Source |
|---|---|---|---|
| 1 | Plugin Subprocess Command Injection / Arbitrary File Read | CRITICAL | KB T-01, Candidate Score 90 (command-execution) |
| 2 | Browser Extension Content Script DOM Injection | MEDIUM | KB T-05, Candidate Score 665 (content.js) |

## Finding Summary

| ID | Slug | Severity | Type | Entry Point |
|---|---|---|---|---|
| l4-001 | c-plugin-popen-command-injection | **CRITICAL** | Shell command injection | `plugins/c/crepuscularity_plugin.c:12` |
| l4-002 | cpp-plugin-popen-command-injection | **CRITICAL** | Shell command injection | `plugins/cpp/crepuscularity_plugin.cpp:16` |
| l4-003 | zig-plugin-sh-c-command-injection | **CRITICAL** | Shell command injection | `plugins/zig/crepuscularity.zig:17` |
| l4-004 | all-plugins-arbitrary-file-read | **HIGH** | Path traversal / file read | All plugin bindings |
| l4-005 | cli-native-ir-path-traversal | **HIGH** | Path traversal / file read | `crates/crepuscularity-cli/src/native.rs:236` |
| l4-006 | content-script-iframe-sandbox-script-execution | **MEDIUM** | Iframe script execution | `content.js:342-345` |
| l4-007 | v-plugin-shell-execution-quoted-path | **MEDIUM** | Potential shell injection | `plugins/v/crepuscularity.v:21` |
| l4-008 | php-plugin-dual-exec-path | **MEDIUM** | Partial path validation | `plugins/php/CrepuscularityPlugin.php:35-38` |
| l4-009 | content-script-sanitizehtml-mxxs-surface | **MEDIUM** | Mutation XSS (dual parse) | `content.js:285-324` |
| l4-010 | crepus-bin-env-var-injection | **MEDIUM** | Binary path injection | All plugin bindings |

## Severity Distribution

| Severity | Count |
|---|---|
| CRITICAL | 3 |
| HIGH | 2 |
| MEDIUM | 5 |

## Key Findings

### CRITICAL: Shell Command Injection in C, C++, and Zig Plugin Bindings

Three plugin bindings construct shell commands from unsanitized user-controlled `path` arguments:

- **C plugin** (`plugins/c/crepuscularity_plugin.c:11-12`): `snprintf` → `popen(cmd, "r")`
- **C++ plugin** (`plugins/cpp/crepuscularity_plugin.cpp:15-16`): string concat → `popen(cmd.c_str(), "r")`
- **Zig plugin** (`plugins/zig/crepuscularity.zig:12-17`): `allocPrint` → `/bin/sh -c` via `Child.run`

All three embed the `path` argument into shell command strings without escaping or validation. An attacker providing `path = "\";malicious_command;\""` achieves arbitrary command execution.

**Contrast:** The other 11+ plugin bindings (Python, Go, TypeScript, Ruby, PHP, C#, Java, Kotlin, Swift, Rust, V) use argument list APIs (`subprocess.run([list])`, `exec.Command(args...)`, `ProcessBuilder`, `Open3.capture3(args)`) — these are safe from shell injection but still vulnerable to arbitrary file read.

### HIGH: Arbitrary File Read in ALL Plugin Bindings

Every plugin binding reads the file at the caller-supplied `path` before passing its content to `crepus native ir --stdin-json`. Path traversal (`../../etc/passwd`) works universally because:
1. No plugin validates the path before `File.read(path)`
2. The CLI's `native.rs` does not apply `resolve_include_path`-style validation
3. File content appears in the View IR output or error messages

This is the **single highest-risk unmitigated issue** identified in the Phase L2 knowledge base (T-01).

### MEDIUM: Browser Extension Content Script DOM Injection Surfaces

The extension content script (`content.js`) has two risk vectors:

1. **Iframe script execution bypass**: When the WASM module indicates content needs scripting, the rendered HTML is inserted into a sandboxed iframe (`sandbox="allow-scripts"`) without going through `sanitizeHTML()`. The iframe sandbox prevents parent DOM access but allows arbitrary JavaScript execution.

2. **Mutation XSS in `sanitizeHTML`**: The DOMParser → `innerHTML` dual-parse pattern is a known mXSS class. Mitigated by a restrictive tag allowlist but not eliminated.

## Coverage Assessment

| Attack Surface Area | Covered? | Findings |
|---|---|---|
| Plugin subprocess path validation (all languages) | Yes | l4-001 through l4-005, l4-007, l4-008, l4-010 |
| Browser content script DOM injection | Yes | l4-006, l4-009 |
| Template include path validation (core) | Partial | Not a focus — mitigated by `resolve_include_path` |
| V8 native bridge | No | Out of scope for this probe |
| SSR HTTP handling | No | Out of scope for this probe |

## Recommended Remediation

1. **IMMEDIATE** — Fix C, C++, and Zig plugin bindings to use args-list process execution instead of shell commands. Replace `popen()` with `execvp()`-style APIs; replace `/bin/sh -c` with direct process execution.

2. **IMMEDIATE** — Add path validation to all plugin bindings. Implement a `resolve_plugin_path(base_dir, path)` utility (reuse the pattern from `include_paths.rs::resolve_include_path`) and apply it before `File.read(path)` in every binding.

3. **SHORT-TERM** — Add path validation to CLI `native.rs` `run_ir_inner` for the `path` argument.

4. **SHORT-TERM** — Restrict `CREPUS_BIN` to a validated set of paths or validate the binary before execution.

5. **MEDIUM-TERM** — Add a second layer of sanitization in the content script: apply `sanitizeHTML()` to iframe content as well (even though sandboxed), or verify the WASM module's output independently.

6. **MEDIUM-TERM** — Replace DOMParser-based sanitization with a well-audited library (e.g., DOMPurify) or use Sanitizer API if available.
