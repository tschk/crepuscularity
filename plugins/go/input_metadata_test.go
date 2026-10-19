package crepuscularity

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestInputMetadataSharedFixture(t *testing.T) {
	raw, err := os.ReadFile("../fixtures/input-metadata.view-ir.json")
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
		t.Fatalf("metadata HTML = %q; want %q", actual.String(), expected)
	}
}

func TestRenderHTMLInputMetadataFromCLI(t *testing.T) {
	directory := t.TempDir()
	t.Setenv("CREPUS_ALLOWED_DIR", directory)
	path := filepath.Join(directory, "metadata.crepus")
	source := "input type=email name={field_name} required={true} value={initial}\ninput name=\"\" required={false}\ntextarea name=notes required={true} value={notes}"
	if err := os.WriteFile(path, []byte(source), 0600); err != nil {
		t.Fatal(err)
	}
	context := map[string]any{"field_name": "mail\"<&雪", "initial": "a@example.test", "notes": "\nfirst\rsecond"}
	expected := `<input type="email" data-bind="" placeholder="" name="mail&#34;&lt;&amp;雪" required value="a@example.test"><input data-bind="" placeholder="" name=""><textarea data-bind="" placeholder="" name="notes" required>` + "\n\nfirst&#13;second" + `</textarea>`
	actual, err := RenderHTML(path, context)
	if err != nil {
		t.Fatal(err)
	}
	if actual != expected {
		t.Fatalf("public HTML = %q; want %q", actual, expected)
	}
	actual, err = NewViewSession(path, context).RenderHTML()
	if err != nil {
		t.Fatal(err)
	}
	if actual != expected {
		t.Fatalf("session HTML = %q; want %q", actual, expected)
	}
}
