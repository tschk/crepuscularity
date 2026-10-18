require "minitest/autorun"
require "tmpdir"
require "rbconfig"
require_relative "../crepuscularity_plugin"

class TestInputDefaultsHtml < Minitest::Test
  def test_public_html_retains_defaults_and_bound_control_precedence
    fixture = JSON.parse(File.read(File.expand_path("../../fixtures/input-defaults.view-ir.json", __dir__)))
    # Labelled CLI stub isolates the public adapter from the Rust parser.
    Dir.mktmpdir("crepus-defaults-") do |dir|
      File.write(File.join(dir, "defaults.crepus"), "input value=\"Ada\"\n")
      File.write(File.join(dir, "crepus"), "#!#{RbConfig.ruby}\nSTDOUT.write(#{JSON.generate(fixture).dump})\n")
      File.chmod(0o755, File.join(dir, "crepus"))
      old_path, old_bin = ENV["PATH"], ENV["CREPUS_BIN"]
      begin
        ENV["PATH"] = "#{dir}#{File::PATH_SEPARATOR}#{old_path}"
        ENV["CREPUS_BIN"] = "crepus"
        actual = Dir.chdir(dir) { CrepuscularityPlugin.render_html("defaults.crepus") }
        assert_equal fixture["expectedHtml"], actual
        File.write(ENV["CREPUS_HTML_OUTPUT"], actual) if ENV["CREPUS_HTML_OUTPUT"]
      ensure
        ENV["PATH"], ENV["CREPUS_BIN"] = old_path, old_bin
      end
    end
  end
end
