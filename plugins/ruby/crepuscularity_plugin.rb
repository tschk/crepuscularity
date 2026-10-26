require "json"
require "open3"
require "cgi"

module CrepuscularityPlugin
  ViewIr = Struct.new(:version, :root, keyword_init: true)

  BIND_BLOCKLIST = ["baseDir", "_"].freeze # ponytail: block security-sensitive keys only

  HTML_INPUT_TYPES = %w[text search tel url email password number date datetime-local month time week color hidden].freeze

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
    unless resolved_path.start_with?(File.join(base_dir, "")) || resolved_path == base_dir
      raise ArgumentError, "Invalid path: must be within current directory"
    end

    bin = ENV.fetch("CREPUS_BIN", "crepus")
    raise SecurityError, "CREPUS_BIN must be strictly a binary name without directory separators" if bin.match?(%r{[/\\]})
    if context
      payload = JSON.generate({
        "template" => File.read(path, encoding: Encoding::UTF_8),
        "filename" => path,
        "context" => context,
        "baseDir" => File.dirname(path)
      })
      stdout, stderr, status = Open3.capture3(bin, "native", "ir", "--stdin-json", stdin_data: payload, binmode: true)
    else
      stdout, stderr, status = Open3.capture3(bin, "native", "ir", path, binmode: true)
    end
    (status.success? ? [stdout, stderr] : [stderr]).each do |stream|
      stream.force_encoding(Encoding::UTF_8)
      unless stream.valid_encoding?
        raise Encoding::InvalidByteSequenceError, "CLI output must contain valid UTF-8"
      end
    end
    raise stderr unless status.success?
    data = JSON.parse(stdout)
    ViewIr.new(version: data.fetch("version"), root: data.fetch("root"))
  end

  def self.render_html(path, context = nil)
    render_ir(path, context).root.map { |node| render_node(node) }.join
  end

  def self.safe_link_href?(value)
    # Match browser scheme parsing; entity literals are escaped by the caller.
    normalized = value.sub(/\A[\x00-\x20]+/, "").delete("\t\r\n")
    !normalized.include?(":") || !%w[javascript vbscript data].include?(normalized.split(":", 2).first.tr("A-Z", "a-z"))
  end

  def self.escape_attr(value)
    CGI.escapeHTML(value).gsub("\r", "&#13;")
  end
  private_class_method :escape_attr

  def self.render_node(node)
    case node["kind"]
    when "text"
      CGI.escapeHTML(node.fetch("content", "").to_s)
    when "stack", "scroll"
      children = node.fetch("children", []).map { |child| render_node(child) }.join
      %(<div data-crepus-kind="#{escape_attr(node["kind"].to_s)}" data-axis="#{escape_attr(node.fetch("axis", "column").to_s)}">#{children}</div>)
    when "button"
      label = CGI.escapeHTML(node.fetch("label", "").to_s)
      node["onClick"] ? %(<button data-onclick="#{escape_attr(node["onClick"].to_s)}">#{label}</button>) : "<button>#{label}</button>"
    when "link"
      href = node.fetch("href", "").to_s
      attrs = safe_link_href?(href) ? %( href="#{escape_attr(href)}") : ""
      %w[target rel].each do |name|
        attrs += %( #{name}="#{escape_attr(node[name].to_s)}") unless node[name].nil?
      end
      children = node.fetch("children", []).map { |child| render_node(child) }.join
      %(<a#{attrs}>#{children}</a>)
    when "image"
      %(<img src="#{escape_attr(node.fetch("src", "").to_s)}" alt="#{escape_attr(node.fetch("alt", "").to_s)}">)
    when "slotRotate"
      %(<span data-crepus-kind="slotRotate">#{CGI.escapeHTML(node.fetch("phrases", [""]).first.to_s)}</span>)
    when "input"
      bind = escape_attr(node.fetch("bind", "").to_s)
      placeholder = escape_attr(node.fetch("placeholder", "").to_s)
      style = node["style"] || {}
      input_type = node["secure"] ? "password" : style["inputType"]
      type_attr = HTML_INPUT_TYPES.include?(input_type) ? %( type="#{input_type}") : ""
      metadata = style["formName"].is_a?(String) ? %( name="#{escape_attr(style["formName"])}") : ""
      metadata += " required" if style["required"].equal?(true)
      value = style["formValue"] if bind.empty?
      if node["multiline"]
        text = value.nil? ? "" : CGI.escapeHTML(value.to_s).gsub("\r", "&#13;")
        # HTML discards one initial LF; encode CR to retain it through parsing.
        text = "\n" + text if text.start_with?("\n")
        %(<textarea data-bind="#{bind}" placeholder="#{placeholder}"#{metadata}>#{text}</textarea>)
      else
        initial = value.nil? ? "" : %( value="#{escape_attr(value.to_s)}")
        %(<input#{type_attr} data-bind="#{bind}" placeholder="#{placeholder}"#{metadata}#{initial}>)
      end
    when "picker"
      options = node.fetch("options", []).map do |option|
        %(<option value="#{escape_attr(option.fetch("value", "").to_s)}">#{CGI.escapeHTML(option.fetch("label", "").to_s)}</option>)
      end.join
      %(<select data-bind="#{escape_attr(node.fetch("bind", "").to_s)}">#{options}</select>)
    else
      ""
    end
  end
end
