package crepuscularity

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestInputDefaultsSharedFixture(t *testing.T) {
	raw, err := os.ReadFile("../fixtures/input-defaults.view-ir.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixture struct {
		Root         []map[string]any `json:"root"`
		ExpectedHTML string           `json:"expectedHtml"`
	}
	if err := json.Unmarshal(raw, &fixture); err != nil {
		t.Fatal(err)
	}
	var actual strings.Builder
	for _, node := range fixture.Root {
		actual.WriteString(renderNode(node))
	}
	expected := strings.ReplaceAll(fixture.ExpectedHTML, "&quot;", "&#34;")
	if actual.String() != expected {
		t.Fatalf("input defaults HTML = %q; want %q", actual.String(), expected)
	}
	for _, node := range []map[string]any{
		{"kind": "input"},
		{"kind": "input", "style": nil},
		{"kind": "input", "style": map[string]any{"formValue": nil}},
	} {
		if actual := renderNode(node); actual != `<input data-bind="" placeholder="">` {
			t.Fatalf("missing input fields = %q", actual)
		}
	}
	if actual := renderNode(map[string]any{"kind": "input", "multiline": true, "secure": true}); actual != `<textarea data-bind="" placeholder=""></textarea>` {
		t.Fatalf("secure multiline fallback = %q", actual)
	}
}

func TestRenderHTMLInputDefaultsFromCLI(t *testing.T) {
	directory := t.TempDir()
	t.Setenv("CREPUS_ALLOWED_DIR", directory)
	path := filepath.Join(directory, "inputs.crepus")
	if err := os.WriteFile(path, []byte("input type=password value={secret}\ntextarea value={notes}\ninput bind=title value=\"ignored\""), 0600); err != nil {
		t.Fatal(err)
	}
	context := map[string]any{"secret": "secret<&\"雪", "notes": "\nfirst\rsecond", "title": "bound"}
	expected := `<input type="password" data-bind="" placeholder="" value="secret&lt;&amp;&#34;雪"><textarea data-bind="" placeholder="">` + "\n\nfirst&#13;second" + `</textarea><input data-bind="title" placeholder="">`
	actual, err := RenderHTML(path, context)
	if err != nil {
		t.Fatal(err)
	}
	if actual != expected {
		t.Fatalf("public HTML = %q; want %q", actual, expected)
	}
	session := NewViewSession(path, context)
	actual, err = session.RenderHTML()
	if err != nil {
		t.Fatal(err)
	}
	if actual != expected {
		t.Fatalf("session HTML = %q; want %q", actual, expected)
	}
}
