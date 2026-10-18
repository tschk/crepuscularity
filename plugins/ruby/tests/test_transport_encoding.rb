require "minitest/autorun"
require "tmpdir"
require "open3"
require "rbconfig"

# Each public-wrapper call runs in its own ASCII-locale Ruby process.
class TransportEncodingTest < Minitest::Test
  def fixture
    skip "Unix executable fixture" if Gem.win_platform?
    Dir.mktmpdir("crepus-ruby-encoding-") do |directory|
      stub = <<~RUBY
        #!#{RbConfig.ruby}
        require "json"
        File.write("transport-called", "yes")
        if ARGV.include?("--stdin-json")
          payload = JSON.parse(STDIN.binmode.read)
          abort "template changed" unless payload.fetch("template").include?([0x96ea].pack("U"))
        end
        STDOUT.write(JSON.generate({version: 7, root: [{kind: "text", content: [0x96ea].pack("U")}]}))
      RUBY
      File.write(File.join(directory, "crepus"), stub, encoding: Encoding::UTF_8)
      File.chmod(0o755, File.join(directory, "crepus"))
      yield directory
    end
  end

  def child(directory, code)
    environment = {"LC_ALL" => "C", "LANG" => "C", "RUBYOPT" => nil,
                   "CREPUS_BIN" => "crepus", "PATH" => "#{directory}#{File::PATH_SEPARATOR}#{ENV.fetch('PATH', '')}"}
    library = File.expand_path("..", __dir__)
    Open3.capture3(environment, RbConfig.ruby, "-I#{library}", "-e", code, chdir: directory)
  end

  def test_valid_utf8_template_with_and_without_context
    fixture do |directory|
      File.write(File.join(directory, "view.crepus"), "span \"雪\"", encoding: Encoding::UTF_8)
      ["nil", "{}"].each do |context|
        code = <<~RUBY
          require "crepuscularity_plugin"
          result = CrepuscularityPlugin.render_ir("view.crepus", #{context})
          raise "Unicode changed" unless result.root[0].fetch("content") == [0x96ea].pack("U")
        RUBY
        _stdout, stderr, status = child(directory, code)
        assert status.success?, "context=#{context}: #{stderr}"
      end
    end
  end

  def test_invalid_utf8_template_is_rejected_before_transport
    fixture do |directory|
      File.binwrite(File.join(directory, "view.crepus"), "\xff".b)
      code = <<~RUBY
        require "crepuscularity_plugin"
        begin
          CrepuscularityPlugin.render_ir("view.crepus", {})
        rescue JSON::GeneratorError, Encoding::InvalidByteSequenceError
          exit 0
        end
        abort "invalid UTF-8 accepted"
      RUBY
      _stdout, stderr, status = child(directory, code)
      assert status.success?, stderr
      refute File.exist?(File.join(directory, "transport-called")), "CLI spawned for invalid source"
    end
  end
  def test_invalid_utf8_stdout_is_rejected_even_inside_quoted_json
    fixture do |directory|
      File.write(File.join(directory, "view.crepus"), 'span "ASCII"')
      File.write(File.join(directory, "crepus"), <<~RUBY)
        #!#{RbConfig.ruby}
        STDOUT.binmode
        STDOUT.write('{"version":7,"root":[{"kind":"text","content":"'.b + [255].pack("C") + '"}]}'.b)
      RUBY
      code = <<~RUBY
        require "crepuscularity_plugin"
        begin
          CrepuscularityPlugin.render_ir("view.crepus")
        rescue Encoding::InvalidByteSequenceError, JSON::ParserError
          exit 0
        end
        abort "invalid UTF-8 stdout accepted"
      RUBY
      _stdout, stderr, status = child(directory, code)
      assert status.success?, stderr
    end
  end

  def test_unicode_cli_error_message_uses_utf8_under_ascii_locale
    fixture do |directory|
      File.write(File.join(directory, "view.crepus"), 'span "ASCII"')
      File.write(File.join(directory, "crepus"), <<~RUBY)
        #!#{RbConfig.ruby}
        STDERR.binmode
        STDERR.write([0x96ea, 0x20, 0x65, 0x72, 0x72, 0x6f, 0x72].pack("U*"))
        exit 1
      RUBY
      code = <<~RUBY
        require "crepuscularity_plugin"
        begin
          CrepuscularityPlugin.render_ir("view.crepus")
        rescue RuntimeError => error
          expected = [0x96ea, 0x20, 0x65, 0x72, 0x72, 0x6f, 0x72].pack("U*")
          raise "wrong encoding" unless error.message.encoding == Encoding::UTF_8
          raise "diagnostic changed" unless error.message == expected
          exit 0
        end
        abort "CLI error accepted"
      RUBY
      _stdout, stderr, status = child(directory, code)
      assert status.success?, stderr
    end
  end

  def test_nonzero_exit_preserves_unicode_stderr_despite_invalid_stdout
    fixture do |directory|
      File.write(File.join(directory, "view.crepus"), 'span "ASCII"')
      # Labelled CLI transport stub: exact invalid stdout bytes and valid UTF-8 stderr.
      File.write(File.join(directory, "crepus"), <<~RUBY)
        #!#{RbConfig.ruby}
        STDIN.binmode.read if ARGV.include?("--stdin-json")
        STDOUT.binmode.write([255].pack("C"))
        STDERR.binmode.write([0x96ea, 0x20, 0x65, 0x72, 0x72, 0x6f, 0x72].pack("U*"))
        exit 9
      RUBY
      ["nil", "{}"].each do |context|
        code = <<~RUBY
          require "crepuscularity_plugin"
          begin
            CrepuscularityPlugin.render_ir("view.crepus", #{context})
          rescue RuntimeError => error
            expected = [0x96ea, 0x20, 0x65, 0x72, 0x72, 0x6f, 0x72].pack("U*")
            raise "wrong encoding" unless error.message.encoding == Encoding::UTF_8
            raise "diagnostic changed" unless error.message == expected
            exit 0
          end
          abort "CLI failure accepted"
        RUBY
        _stdout, stderr, status = child(directory, code)
        assert status.success?, "context=#{context}: #{stderr}"
      end
    end
  end

end
