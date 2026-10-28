require "minitest/autorun"
require "tmpdir"
require "rbconfig"
require_relative "../crepuscularity_plugin"

class TestLinkHtml < Minitest::Test
  def test_public_html_preserves_links_attributes_and_children
    # Labelled CLI stub isolates the public HTML adapter from the Rust compiler.
    nodes = [
      { "kind" => "stack", "children" => [
        { "kind" => "link", "href" => '/雪?q="<&', "hrefBind" => "scope.next", "target" => '"_blank', "rel" => 'next & "safe"',
          "children" => [{ "kind" => "text", "content" => 'Help 雪 <&"' },
                           { "kind" => "image", "src" => '/icon?x="', "alt" => "icon" }] }
      ] },
      { "kind" => "link", "href" => "", "target" => "", "rel" => nil,
        "children" => [{ "kind" => "text", "content" => "Empty" }] },
      { "kind" => "link", "href" => "/plain", "children" => [] }
    ]
    hrefs = [
      "JaVaScRiPt:alert(1)",
      "\x00 \tjava\nscript:alert(1)",
      "vB\rScRiPt:bad",
      "\x1fDaTa:text/html,bad",
      "&#106;avascript:literal",
      "/relative?q=1&x=2",
      "https://example.test/雪",
      "mailto:user@example.test"
    ]
    nodes.concat(hrefs.map do |href|
      { "kind" => "link", "href" => href,
        "children" => [{ "kind" => "text", "content" => "safe" }] }
    end)
    Dir.mktmpdir("crepus-link-") do |dir|
      File.write(File.join(dir, "input.crepus"), 'a href="/help"' + "\n" + '  "Help"' + "\n")
      File.write(File.join(dir, "crepus"), "#!#{RbConfig.ruby}\nSTDOUT.write(#{JSON.generate({ version: 7, root: nodes }).dump})\n")
      File.chmod(0o755, File.join(dir, "crepus"))
      old_path, old_bin = ENV["PATH"], ENV["CREPUS_BIN"]
      begin
        ENV["PATH"] = "#{dir}#{File::PATH_SEPARATOR}#{old_path}"
        ENV["CREPUS_BIN"] = "crepus"
        Dir.chdir(dir) do
          assert_equal '<div data-crepus-kind="stack" data-axis="column"><a href="/雪?q=&quot;&lt;&amp;" target="&quot;_blank" rel="next &amp; &quot;safe&quot;">Help 雪 &lt;&amp;&quot;<img src="/icon?x=&quot;" alt="icon"></a></div><a href="" target="">Empty</a><a href="/plain"></a>' + "<a>safe</a><a>safe</a><a>safe</a><a>safe</a><a href=\"&amp;#106;avascript:literal\">safe</a><a href=\"/relative?q=1&amp;x=2\">safe</a><a href=\"https://example.test/雪\">safe</a><a href=\"mailto:user@example.test\">safe</a>",
                       CrepuscularityPlugin.render_html("input.crepus")
        end
      ensure
        ENV["PATH"], ENV["CREPUS_BIN"] = old_path, old_bin
      end
    end
  end
end
