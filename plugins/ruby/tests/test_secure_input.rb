require "minitest/autorun"
require "tmpdir"
require "rbconfig"
require_relative "../crepuscularity_plugin"

class TestSecureInputHtml < Minitest::Test
  def test_public_html_preserves_secure_plain_and_multiline_inputs
    # Labelled CLI stub isolates the public HTML adapter from the Rust compiler.
    nodes = [
      { "kind" => "input", "secure" => true, "bind" => "password", "placeholder" => "" },
      { "kind" => "input", "secure" => false, "bind" => "plain", "placeholder" => "" },
      { "kind" => "input", "secure" => true, "multiline" => true, "bind" => "notes", "placeholder" => "" }
    ]
    Dir.mktmpdir("crepus-secure-") do |dir|
      File.write(File.join(dir, "input.crepus"), "input secure bind=password\n")
      File.write(File.join(dir, "crepus"), "#!#{RbConfig.ruby}\nSTDOUT.write(#{JSON.generate({ version: 7, root: nodes }).dump})\n")
      File.chmod(0o755, File.join(dir, "crepus"))
      old_path, old_bin = ENV["PATH"], ENV["CREPUS_BIN"]
      begin
        ENV["PATH"] = "#{dir}#{File::PATH_SEPARATOR}#{old_path}"
        ENV["CREPUS_BIN"] = "crepus"
        Dir.chdir(dir) do
          assert_equal '<input type="password" data-bind="password" placeholder=""><input data-bind="plain" placeholder=""><textarea data-bind="notes" placeholder=""></textarea>',
                       CrepuscularityPlugin.render_html("input.crepus")
        end
      ensure
        ENV["PATH"], ENV["CREPUS_BIN"] = old_path, old_bin
      end
    end
  end
end
