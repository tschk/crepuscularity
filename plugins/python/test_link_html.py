import unittest
from unittest.mock import patch

from crepuscularity_plugin import ViewIr, ViewSession, render_html


class LinkHtmlTests(unittest.TestCase):
    def test_public_html_preserves_attributes_and_recursive_children(self):
        nodes = [
            {"kind": "stack", "children": [
                {"kind": "link", "href": '/雪?q="<&', "hrefBind": "scope.next", "target": '"_blank',
                 "rel": 'next & "safe"', "children": [
                     {"kind": "text", "content": 'Help 雪 <&"'},
                     {"kind": "image", "src": '/icon?x="', "alt": "icon"}]}]},
            {"kind": "link", "href": "", "target": "", "rel": None,
             "children": [{"kind": "text", "content": "Empty"}]},
            {"kind": "link", "href": "/plain", "children": []},
        ]
        hrefs = [
            "JaVaScRiPt:alert(1)",
            "\x00 \tjava\nscript:alert(1)",
            "vB\rScRiPt:bad",
            "\x1fDaTa:text/html,bad",
            "&#106;avascript:literal",
            "/relative?q=1&x=2",
            "https://example.test/雪",
            "mailto:user@example.test",
        ]
        nodes.extend({"kind": "link", "href": href,
                      "children": [{"kind": "text", "content": "safe"}]}
                     for href in hrefs)
        expected = ('<div data-crepus-kind="stack" data-axis="column">'
                    '<a href="/雪?q=&quot;&lt;&amp;" target="&quot;_blank" '
                    'rel="next &amp; &quot;safe&quot;">Help 雪 &lt;&amp;&quot;'
                    '<img src="/icon?x=&quot;" alt="icon"></a></div>'
                    '<a href="" target="">Empty</a><a href="/plain"></a>')
        expected += '<a>safe</a><a>safe</a><a>safe</a><a>safe</a><a href="&amp;#106;avascript:literal">safe</a><a href="/relative?q=1&amp;x=2">safe</a><a href="https://example.test/雪">safe</a><a href="mailto:user@example.test">safe</a>'
        # Labelled IR collaborator: tests the actual public adapter, not the CLI parser.
        with patch("crepuscularity_plugin.render_ir", return_value=ViewIr(7, nodes)):
            self.assertEqual(render_html("links.crepus"), expected)
            self.assertEqual(ViewSession("links.crepus").render_html(), expected)


if __name__ == "__main__":
    unittest.main()
