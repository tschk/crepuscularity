require "minitest/autorun"
require "tmpdir"
require "rbconfig"
require_relative "../crepuscularity_plugin"

class TestInputMetadataHtml < Minitest::Test
  def test_public_html_retains_input_types_names_and_required
    fixture = JSON.parse(File.read(File.expand_path("../../fixtures/input-metadata.view-ir.json", __dir__)))
    # Labelled CLI stub isolates the public adapter from the Rust parser.
    Dir.mktmpdir("crepus-metadata-") do |dir|
      File.write(File.join(dir, "metadata.crepus"), "input value=\"Ada\"\n")
      File.write(File.join(dir, "crepus"), "#!#{RbConfig.ruby}\nSTDOUT.write(#{JSON.generate(fixture).dump})\n")
      File.chmod(0o755, File.join(dir, "crepus"))
      old_path, old_bin = ENV["PATH"], ENV["CREPUS_BIN"]
      begin
        ENV["PATH"] = "#{dir}#{File::PATH_SEPARATOR}#{old_path}"
        ENV["CREPUS_BIN"] = "crepus"
        actual = Dir.chdir(dir) { CrepuscularityPlugin.render_html("metadata.crepus") }
        assert_equal fixture["expectedHtml"], actual
        session_html = Dir.chdir(dir) { CrepuscularityPlugin::ViewSession.new("metadata.crepus").render_html }
        assert_equal actual, session_html
        File.write(ENV["CREPUS_HTML_OUTPUT"], actual) if ENV["CREPUS_HTML_OUTPUT"]
      ensure
        ENV["PATH"], ENV["CREPUS_BIN"] = old_path, old_bin
      end
    end
  end
end
