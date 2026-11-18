require "minitest/autorun"
require_relative "../crepuscularity_plugin"

class TestCrepuscularityPluginSecurity < Minitest::Test
  def setup
    @secret_file = "/tmp/test-secret-#{$$}.txt"
    File.write(@secret_file, "super secret content")

    @symlink_file = "plugins/ruby/symlink-#{$$}.crepus"
    File.symlink("/etc/passwd", @symlink_file)
  end

  def teardown
    File.delete(@secret_file) if File.exist?(@secret_file)
    File.delete(@symlink_file) if File.exist?(@symlink_file)
  end

  def test_arbitrary_file_read_blocked
    assert_raises(ArgumentError) do
      CrepuscularityPlugin.render_ir(@secret_file, { "key" => "value" })
    end
  end

  def test_path_traversal_blocked
    assert_raises(ArgumentError) do
      CrepuscularityPlugin.render_ir("../../../../../etc/passwd", { "key" => "value" })
    end
  end

  def test_symlink_bypass_blocked
    # Symlinks pointing outside the pwd should be blocked even if they are in a valid directory
    assert_raises(ArgumentError) do
      CrepuscularityPlugin.render_ir(@symlink_file, { "key" => "value" })
    end
  end

  def test_valid_path_allowed
    # Test with a valid local path
    valid_file = "plugins/ruby/fixtures/interactive.crepus"
    skip "Fixture not found" unless File.exist?(valid_file)

    # Should not raise
    ir = CrepuscularityPlugin.render_ir(valid_file, { "count" => "1" })
    assert ir.root
  end
end
