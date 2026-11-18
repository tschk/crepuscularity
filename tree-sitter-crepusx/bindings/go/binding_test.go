package tree_sitter_crepusx_test

import (
	"testing"

	tree_sitter "github.com/smacker/go-tree-sitter"
	"github.com/softwarecorporation/crepuscularity/tree-sitter-crepusx"
)

func TestCanLoadGrammar(t *testing.T) {
	language := tree_sitter.NewLanguage(tree_sitter_crepusx.Language())
	if language == nil {
		t.Errorf("Error loading CrepusX grammar")
	}
}
