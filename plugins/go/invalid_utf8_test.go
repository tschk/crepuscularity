package crepuscularity

import (
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

// The executable is a labelled transport stub; no compiler behavior is simulated.
func utf8TransportFixture(t *testing.T) (string, string) {
	t.Helper()
	if runtime.GOOS == "windows" {
		t.Skip("transport fixture uses a Unix shell executable")
	}
	directory := t.TempDir()
	capture := filepath.Join(directory, "stdin.json")
	binary := filepath.Join(directory, "crepus")
	script := "#!/bin/sh\ncat >\"$CREPUS_TEST_CAPTURE\"\nprintf '%s' '{\"version\":7,\"root\":[]}'\n"
	if err := os.WriteFile(binary, []byte(script), 0700); err != nil {
		t.Fatal(err)
	}
	t.Setenv("CREPUS_BIN", binary)
	t.Setenv("CREPUS_ALLOWED_DIR", directory)
	t.Setenv("CREPUS_TEST_CAPTURE", capture)
	return directory, capture
}

func TestRenderIRRejectsInvalidUTF8BeforeSpawningCLI(t *testing.T) {
	for _, source := range [][]byte{{0xff}, {'s', 'p', 'a', 'n', ' ', '"', 0xc3, 0x28, '"'}} {
		t.Run(string(source), func(t *testing.T) {
			directory, capture := utf8TransportFixture(t)
			path := filepath.Join(directory, "view.crepus")
			if err := os.WriteFile(path, source, 0600); err != nil {
				t.Fatal(err)
			}
			_, err := RenderIR(path, map[string]any{})
			if err == nil || !strings.Contains(strings.ToLower(err.Error()), "utf-8") {
				t.Fatalf("expected explicit UTF-8 error, got %v", err)
			}
			if _, err := os.Stat(capture); !os.IsNotExist(err) {
				t.Fatalf("CLI transport was invoked before validation: %v", err)
			}
		})
	}
}

func TestRenderIRPreservesValidUnicodeInTransport(t *testing.T) {
	directory, capture := utf8TransportFixture(t)
	path := filepath.Join(directory, "view.crepus")
	source := "span \"雪 🦋\"\n"
	if err := os.WriteFile(path, []byte(source), 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := RenderIR(path, map[string]any{}); err != nil {
		t.Fatal(err)
	}
	raw, err := os.ReadFile(capture)
	if err != nil {
		t.Fatal(err)
	}
	var payload struct {
		Template string `json:"template"`
	}
	if err := json.Unmarshal(raw, &payload); err != nil {
		t.Fatal(err)
	}
	if payload.Template != source {
		t.Fatalf("template changed: %q", payload.Template)
	}
}

func TestRenderIRValidatesCLIResponseUTF8(t *testing.T) {
	for _, test := range []struct {
		name    string
		content []byte
		invalid bool
	}{
		{"invalid byte", []byte{0xff}, true},
		{"malformed sequence", []byte{0xc3, 0x28}, true},
		{"raw Unicode", []byte("雪 🦋"), false},
	} {
		t.Run(test.name, func(t *testing.T) {
			directory, capture := utf8TransportFixture(t)
			path := filepath.Join(directory, "view.crepus")
			if err := os.WriteFile(path, []byte("span \"valid\"\n"), 0600); err != nil {
				t.Fatal(err)
			}
			response := append([]byte(`{"version":7,"root":[{"kind":"text","content":"`), test.content...)
			response = append(response, []byte(`"}]}`)...)
			if err := os.WriteFile(filepath.Join(directory, "response.json"), response, 0600); err != nil {
				t.Fatal(err)
			}
			// Emit exact response bytes from a labelled CLI transport stub.
			script := "#!/bin/sh\ncat >\"$CREPUS_TEST_CAPTURE\"\ncat \"$CREPUS_TEST_RESPONSE\"\n"
			if err := os.WriteFile(filepath.Join(directory, "crepus"), []byte(script), 0700); err != nil {
				t.Fatal(err)
			}
			t.Setenv("CREPUS_TEST_RESPONSE", filepath.Join(directory, "response.json"))
			ir, err := RenderIR(path, map[string]any{})
			if _, captureErr := os.Stat(capture); captureErr != nil {
				t.Fatalf("CLI transport did not run: %v", captureErr)
			}
			if test.invalid {
				if err == nil || !strings.Contains(strings.ToLower(err.Error()), "utf-8") {
					t.Fatalf("expected explicit response UTF-8 error, got %v", err)
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			if len(ir.Root) != 1 || ir.Root[0]["content"] != string(test.content) {
				t.Fatalf("CLI response text changed: %#v", ir.Root)
			}
		})
	}
}
