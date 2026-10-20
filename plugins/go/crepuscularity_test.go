package crepuscularity

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestRenderIRMissingFileReturnsError(t *testing.T) {
	directory := t.TempDir()
	directory, err := filepath.EvalSymlinks(directory)
	if err != nil {
		t.Fatal(err)
	}
	t.Setenv("CREPUS_ALLOWED_DIR", directory)
	_, err = RenderIR(filepath.Join(directory, "missing.crepus"), nil)
	if !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("expected missing file error, got %v", err)
	}
}

func TestRenderIRPreservesFrontendFilename(t *testing.T) {
	t.Setenv("CREPUS_ALLOWED_DIR", "..")
	ir, err := RenderIR("../fixtures/frontend.svelte", map[string]any{"name": "Ada"})
	if err != nil {
		t.Fatal(err)
	}
	text := fmt.Sprint(ir.Root)
	if !strings.Contains(text, "Frontend content") || strings.Contains(text, "ignored") {
		t.Fatalf("frontend output = %s", text)
	}
}

func TestRenderIRRelativeIncludes(t *testing.T) {
	directory := t.TempDir()
	t.Setenv("CREPUS_ALLOWED_DIR", directory)
	path := filepath.Join(directory, "index.crepus")
	if err := os.WriteFile(path, []byte("include part.crepus name={name}\n"), 0600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(directory, "part.crepus"), []byte("span\n  \"Included {name}\"\n"), 0600); err != nil {
		t.Fatal(err)
	}
	ir, err := RenderIR(path, map[string]any{"name": "Ada"})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(fmt.Sprint(ir.Root), "Included Ada") {
		t.Fatalf("include output = %v", ir.Root)
	}
}

func TestRenderIRAllowsFilesystemRoot(t *testing.T) {
	path := filepath.Join(t.TempDir(), "root.crepus")
	if err := os.WriteFile(path, []byte("span\n  \"Root allowed\"\n"), 0600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("CREPUS_ALLOWED_DIR", filepath.VolumeName(path)+string(filepath.Separator))
	ir, err := RenderIR(path, map[string]any{})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(fmt.Sprint(ir.Root), "Root allowed") {
		t.Fatalf("root output = %v", ir.Root)
	}
}

func TestRenderIRAcceptsNilContext(t *testing.T) {
	t.Setenv("CREPUS_ALLOWED_DIR", "..")
	ir, err := RenderIR("../fixtures/hello.crepus", nil)
	if err != nil {
		t.Fatal(err)
	}
	if ir.Version != 7 || !strings.Contains(fmt.Sprint(ir.Root), "Hello ") {
		t.Fatalf("nil context output = %v", ir)
	}
}

func TestRenderIRSeparatesDiagnosticsFromJSON(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("fixture executable requires a POSIX shell")
	}
	directory := t.TempDir()
	t.Setenv("CREPUS_ALLOWED_DIR", directory)
	path := filepath.Join(directory, "index.crepus")
	if err := os.WriteFile(path, []byte("span\n"), 0600); err != nil {
		t.Fatal(err)
	}
	binary := filepath.Join(directory, "crepus")
	t.Setenv("CREPUS_BIN", binary)
	script := "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' 'diagnostic' >&2\nprintf '%s\\n' '{\"version\":7,\"root\":[]}'\n"
	if err := os.WriteFile(binary, []byte(script), 0700); err != nil {
		t.Fatal(err)
	}
	ir, err := RenderIR(path, nil)
	if err != nil || ir.Version != 7 {
		t.Fatalf("diagnostic corrupted JSON: ir=%v error=%v", ir, err)
	}
	if err := os.WriteFile(binary, []byte(script+"exit 3\n"), 0700); err != nil {
		t.Fatal(err)
	}
	_, err = RenderIR(path, nil)
	if err == nil || !strings.Contains(err.Error(), "diagnostic") {
		t.Fatalf("failure diagnostic was lost: %v", err)
	}
}

func TestRenderIRRejectsSiblingPrefixesAndSymlinkEscapes(t *testing.T) {
	root := t.TempDir()
	allowed := filepath.Join(root, "allowed")
	sibling := filepath.Join(root, "allowed-extra")
	for _, directory := range []string{allowed, sibling} {
		if err := os.Mkdir(directory, 0700); err != nil {
			t.Fatal(err)
		}
	}
	outside := filepath.Join(sibling, "outside.crepus")
	if err := os.WriteFile(outside, []byte("span\n  \"Outside\"\n"), 0600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("CREPUS_ALLOWED_DIR", allowed)
	assertDenied := func(t *testing.T, path string) {
		t.Helper()
		_, err := RenderIR(path, nil)
		if err == nil || !strings.Contains(err.Error(), "path traversal denied") {
			t.Fatalf("expected traversal rejection for %s, got %v", path, err)
		}
	}
	t.Run("sibling prefix", func(t *testing.T) { assertDenied(t, outside) })
	t.Run("symlink escape", func(t *testing.T) {
		link := filepath.Join(allowed, "escape.crepus")
		if err := os.Symlink(outside, link); err != nil {
			if runtime.GOOS == "windows" {
				t.Skipf("symlink creation unavailable: %v", err)
			}
			t.Fatal(err)
		}
		assertDenied(t, link)
	})
}

func TestCrepusBinValidation(t *testing.T) {
	orig := os.Getenv("CREPUS_BIN")
	defer os.Setenv("CREPUS_BIN", orig)

	// test absolute path
	absPath := "/usr/bin/crepus"
	if filepath.Separator == '\\' {
		absPath = "C:\\bin\\crepus"
	}
	os.Setenv("CREPUS_BIN", absPath)
	if crepusBin() != absPath {
		t.Fatalf("expected %s, got %s", absPath, crepusBin())
	}

	// test simple binary name
	simple := "mycrepus"
	os.Setenv("CREPUS_BIN", simple)
	if crepusBin() != simple {
		t.Fatalf("expected %s, got %s", simple, crepusBin())
	}

	// test relative path
	relPath := "./crepus"
	os.Setenv("CREPUS_BIN", relPath)
	if crepusBin() != "crepus" { // falls back to default
		t.Fatalf("expected crepus, got %s", crepusBin())
	}
}

func TestRenderIR(t *testing.T) {
	os.Setenv("CREPUS_ALLOWED_DIR", "..")
	ir, err := RenderIR("../fixtures/hello.crepus", map[string]any{"name": "Ada"})
	if err != nil {
		t.Fatal(err)
	}
	if ir.Version != 7 {
		t.Fatalf("version = %d", ir.Version)
	}
	if len(ir.Root) != 1 {
		t.Fatalf("root len = %d", len(ir.Root))
	}
	html, err := RenderHTML("../fixtures/hello.crepus", map[string]any{"name": "Ada"})
	if err != nil {
		t.Fatal(err)
	}
	if html != `<div data-crepus-kind="stack" data-axis="column">Hello Ada</div>` {
		t.Fatalf("html = %s", html)
	}
}

func TestViewSessionDispatch(t *testing.T) {
	os.Setenv("CREPUS_ALLOWED_DIR", "..")
	session := NewViewSession("../fixtures/interactive.crepus", map[string]any{"count": "1"})
	html, err := session.RenderHTML()
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(html, "Count 1") {
		t.Fatalf("html = %s", html)
	}
	ir, err := session.Dispatch(Event{Handler: "bind:count:2"})
	if err != nil {
		t.Fatal(err)
	}
	raw := fmt.Sprint(ir.Root)
	if !strings.Contains(raw, "Count 2") {
		t.Fatalf("ir = %s", raw)
	}
}

func TestRenderIRTraversalDenied(t *testing.T) {
	os.Setenv("CREPUS_ALLOWED_DIR", ".")
	_, err := RenderIR("../fixtures/hello.crepus", map[string]any{"name": "Ada"})
	if err == nil {
		t.Fatal("expected traversal denied error")
	}
	if !strings.Contains(err.Error(), "path traversal denied") {
		t.Fatalf("unexpected error: %v", err)
	}
}
