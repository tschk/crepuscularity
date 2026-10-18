package crepuscularity

import (
	"os"
	"path/filepath"
	"testing"
)

func TestLinkHTMLPreservesChildrenAndEscapesAttributes(t *testing.T) {
	node := map[string]any{"kind": "link", "href": "/雪?q=\"<&", "hrefBind": "scope.next", "target": "\"_blank", "rel": "next & \"safe\"", "children": []any{
		map[string]any{"kind": "text", "content": "Help 雪 <&\""},
		map[string]any{"kind": "stack", "axis": "column", "children": []any{map[string]any{"kind": "button", "label": "Save", "onClick": "save\"&"}}},
	}}
	expected := `<a href="/雪?q=&#34;&lt;&amp;" target="&#34;_blank" rel="next &amp; &#34;safe&#34;">Help 雪 &lt;&amp;&#34;<div data-crepus-kind="stack" data-axis="column"><button data-onclick="save&#34;&amp;">Save</button></div></a>`
	if actual := renderNode(node); actual != expected {
		t.Fatalf("HTML = %q; want %q", actual, expected)
	}
	for _, node := range []map[string]any{
		{"kind": "link"},
		{"kind": "link", "href": "", "rel": nil},
	} {
		if actual := renderNode(node); actual != `<a href=""></a>` {
			t.Fatalf("missing fields = %q", actual)
		}
	}
	if actual := renderNode(map[string]any{"kind": "link", "target": "", "rel": ""}); actual != `<a href="" target="" rel=""></a>` {
		t.Fatalf("empty attributes = %q", actual)
	}
}

func TestLinkHTMLFiltersBrowserDangerousSchemes(t *testing.T) {
	for _, href := range []string{"javascript:alert(1)", "\x00 \tJaVa\nScRiPt:bad()", "VBScript:bad", "data:text/html,bad", "\x1fDATA:bad", "java\rscript:bad"} {
		actual := renderNode(map[string]any{"kind": "link", "href": href, "children": []any{map[string]any{"kind": "text", "content": "safe"}}})
		if actual != `<a>safe</a>` {
			t.Fatalf("unsafe %q became %q", href, actual)
		}
	}
	for _, test := range []struct{ href, want string }{
		{"&#106;avascript:literal", `<a href="&amp;#106;avascript:literal"></a>`},
		{"/relative?q=1&x=2", `<a href="/relative?q=1&amp;x=2"></a>`},
		{"https://example.test/雪", `<a href="https://example.test/雪"></a>`},
		{"mailto:user@example.test", `<a href="mailto:user@example.test"></a>`},
	} {
		if actual := renderNode(map[string]any{"kind": "link", "href": test.href}); actual != test.want {
			t.Fatalf("safe %q became %q", test.href, actual)
		}
	}
}

func TestRenderHTMLLinksFromCLI(t *testing.T) {
	directory := t.TempDir()
	t.Setenv("CREPUS_ALLOWED_DIR", directory)
	path := filepath.Join(directory, "links.crepus")
	if err := os.WriteFile(path, []byte("a href=\"/help?x=1&y=2\" target=\"_blank\" rel=\"help\"\n  span \"Help {name}\""), 0600); err != nil {
		t.Fatal(err)
	}
	context := map[string]any{"name": "Ada <&雪"}
	ir, err := RenderIR(path, context)
	if err != nil {
		t.Fatal(err)
	}
	if len(ir.Root) != 1 || ir.Root[0]["kind"] != "link" {
		t.Fatalf("expected Link IR: %#v", ir)
	}
	expected := `<a href="/help?x=1&amp;y=2" target="_blank" rel="help">Help Ada &lt;&amp;雪</a>`
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
