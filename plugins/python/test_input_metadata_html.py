import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch

from crepuscularity_plugin import ViewIr, ViewSession, render_html


class InputMetadataHtmlTests(unittest.TestCase):
    def test_public_html_retains_input_types_names_and_required(self):
        fixture = json.loads((Path(__file__).parents[1] / "fixtures/input-metadata.view-ir.json").read_text())
        # Labelled IR collaborator isolates the public adapter from the Rust parser.
        with patch("crepuscularity_plugin.render_ir", return_value=ViewIr(fixture["version"], fixture["root"])):
            actual = render_html("metadata.crepus")
            self.assertEqual(actual, fixture["expectedHtml"])
            self.assertEqual(ViewSession("metadata.crepus").render_html(), actual)
        if output := os.environ.get("CREPUS_HTML_OUTPUT"):
            Path(output).write_text(actual)


if __name__ == "__main__":
    unittest.main()
