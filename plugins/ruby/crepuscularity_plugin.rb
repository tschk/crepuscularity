require "json"
require "open3"
require "cgi"

module CrepuscularityPlugin
  ViewIr = Struct.new(:version, :root, keyword_init: true)

  BIND_BLOCKLIST = ["baseDir", "_"].freeze # ponytail: block security-sensitive keys only

  class ViewSession
    attr_reader :path, :context

    def initialize(path, context = {})
      @path = path
      @context = context.dup
      @handlers = {}
    end

    def on(handler, callback = nil, &block)
      @handlers[handler] = callback || block
      self
    end

    def render_ir
      CrepuscularityPlugin.render_ir(@path, @context)
    end

    def render_html
      render_ir.root.map { |node| CrepuscularityPlugin.render_node(node) }.join
    end

    def dispatch(event)
      payload = event.is_a?(String) ? { "handler" => event } : event
      handler = payload.fetch("handler", "").to_s
      if handler.start_with?("bind:")
        key, value = handler.delete_prefix("bind:").split(":", 2)
        @context[key] = value unless value.nil? || BIND_BLOCKLIST.include?(key)
      end
      @handlers[handler]&.call(payload, self)
      render_ir
    end
  end

  def self.render_ir(path, context = nil)
    # resolve symlinks and absolute paths securely
    resolved_path = File.realpath(path)
    # ensure it strictly resides within the current working directory boundary
    base_dir = File.realpath(Dir.pwd)
    unless resolved_path.start_with?(base_dir + File::SEPARATOR) || resolved_path == base_dir
      raise ArgumentError, "Invalid path: must be within current directory"
    end

    bin = ENV.fetch("CREPUS_BIN", "crepus")
    raise SecurityError, "CREPUS_BIN must be strictly a binary name without directory separators" if bin.match?(%r{[/\\]})
    if context
      payload = JSON.generate({
        "template" => File.read(path),
        "context" => context,
        "baseDir" => File.dirname(path)
      })
      stdout, stderr, status = Open3.capture3(bin, "native", "ir", "--stdin-json", stdin_data: payload)
    else
      stdout, stderr, status = Open3.capture3(bin, "native", "ir", path)
    end
    raise stderr unless status.success?
    data = JSON.parse(stdout)
    ViewIr.new(version: data.fetch("version"), root: data.fetch("root"))
  end

  def self.render_html(path, context = nil)
    render_ir(path, context).root.map { |node| render_node(node) }.join
  end

  def self.render_node(node)
    case node["kind"]
    when "text"
      CGI.escapeHTML(node.fetch("content", "").to_s)
    when "stack", "scroll"
      children = node.fetch("children", []).map { |child| render_node(child) }.join
      %(<div data-crepus-kind="#{CGI.escapeHTML(node["kind"].to_s)}" data-axis="#{CGI.escapeHTML(node.fetch("axis", "column").to_s)}">#{children}</div>)
    when "button"
      label = CGI.escapeHTML(node.fetch("label", "").to_s)
      node["onClick"] ? %(<button data-onclick="#{CGI.escapeHTML(node["onClick"].to_s)}">#{label}</button>) : "<button>#{label}</button>"
    when "image"
      %(<img src="#{CGI.escapeHTML(node.fetch("src", "").to_s)}" alt="#{CGI.escapeHTML(node.fetch("alt", "").to_s)}">)
    when "slotRotate"
      %(<span data-crepus-kind="slotRotate">#{CGI.escapeHTML(node.fetch("phrases", [""]).first.to_s)}</span>)
    when "input"
      bind = CGI.escapeHTML(node.fetch("bind", "").to_s)
      placeholder = CGI.escapeHTML(node.fetch("placeholder", "").to_s)
      node["multiline"] ? %(<textarea data-bind="#{bind}" placeholder="#{placeholder}"></textarea>) : %(<input data-bind="#{bind}" placeholder="#{placeholder}">)
    when "picker"
      options = node.fetch("options", []).map do |option|
        %(<option value="#{CGI.escapeHTML(option.fetch("value", "").to_s)}">#{CGI.escapeHTML(option.fetch("label", "").to_s)}</option>)
      end.join
      %(<select data-bind="#{CGI.escapeHTML(node.fetch("bind", "").to_s)}">#{options}</select>)
    else
      ""
    end
  end
end
