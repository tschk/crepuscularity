using System.Diagnostics;
using System.Runtime.ExceptionServices;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Crepuscularity.Plugin;

public sealed record ViewIr(
    [property: JsonPropertyName("version")] int Version,
    [property: JsonPropertyName("root")] JsonElement Root
);

public sealed record EventPayload(string Handler, object? Payload = null);

public sealed class ViewSession
{
    private readonly Dictionary<string, Func<EventPayload, ViewSession, Task>> _handlers = new();

    public ViewSession(string path, IDictionary<string, object?>? context = null)
    {
        Path = path;
        Context = context is null ? new Dictionary<string, object?>() : new Dictionary<string, object?>(context);
    }

    public string Path { get; }

    public Dictionary<string, object?> Context { get; }

    public ViewSession On(string handler, Func<EventPayload, ViewSession, Task> callback)
    {
        _handlers[handler] = callback;
        return this;
    }

    public ViewSession On(string handler, Action<EventPayload, ViewSession> callback)
    {
        _handlers[handler] = (evt, session) =>
        {
            callback(evt, session);
            return Task.CompletedTask;
        };
        return this;
    }

    public Task<ViewIr> RenderIrAsync()
    {
        return Crepuscularity.RenderIrAsync(Path, Context);
    }

    public Task<string> RenderHtmlAsync()
    {
        return Crepuscularity.RenderHtmlAsync(Path, Context);
    }

    public Task<ViewIr> DispatchAsync(string handler)
    {
        return DispatchAsync(new EventPayload(handler));
    }

    public async Task<ViewIr> DispatchAsync(EventPayload evt)
    {
        ApplyBind(evt.Handler);
        if (_handlers.TryGetValue(evt.Handler, out var callback))
        {
            await callback(evt, this);
        }
        return await RenderIrAsync();
    }

    private void ApplyBind(string handler)
    {
        if (!handler.StartsWith("bind:", StringComparison.Ordinal))
        {
            return;
        }
        var rest = handler["bind:".Length..];
        var colon = rest.IndexOf(':');
        if (colon <= 0)
        {
            return;
        }
        Context[rest[..colon]] = rest[(colon + 1)..];
    }
}

public static class Crepuscularity
{
    public static async Task<ViewIr> RenderIrAsync(string path, object? context = null)
    {
        // Match the UTF-8 preamble so BOM detection retains strict decoding.
        var template = await File.ReadAllTextAsync(path, new UTF8Encoding(true, true));
        var payload = JsonSerializer.Serialize(new {
            template,
            filename = path,
            context = context ?? new { },
            baseDir = System.IO.Path.GetDirectoryName(System.IO.Path.GetFullPath(path))
        });
        var bin = Environment.GetEnvironmentVariable("CREPUS_BIN") ?? "crepus";
        using var proc = new Process();
        proc.StartInfo = new ProcessStartInfo(bin, "native ir --stdin-json")
        {
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };
        proc.Start();
        using var stdout = new MemoryStream();
        var stdoutTask = proc.StandardOutput.BaseStream.CopyToAsync(stdout);
        var stderrTask = proc.StandardError.ReadToEndAsync();
        ExceptionDispatchInfo? writeError = null;
        try
        {
            using var stdin = proc.StandardInput;
            await stdin.WriteAsync(payload);
        }
        catch (IOException error)
        {
            writeError = ExceptionDispatchInfo.Capture(error);
        }
        await stdoutTask;
        var stderr = await stderrTask;
        await proc.WaitForExitAsync();
        if (proc.ExitCode != 0)
        {
            throw new InvalidOperationException(stderr);
        }
        writeError?.Throw();
        var bytes = stdout.ToArray();
        var offset = bytes.AsSpan().StartsWith(Encoding.UTF8.Preamble) ? Encoding.UTF8.Preamble.Length : 0;
        var json = new UTF8Encoding(false, true).GetString(bytes, offset, bytes.Length - offset);
        return JsonSerializer.Deserialize<ViewIr>(json) ?? throw new InvalidOperationException("empty IR");
    }

    public static async Task<string> RenderHtmlAsync(string path, object? context = null)
    {
        var ir = await RenderIrAsync(path, context);
        return string.Concat(ir.Root.EnumerateArray().Select(RenderNode));
    }

    private static string EscapeAttribute(string? value) =>
        System.Net.WebUtility.HtmlEncode(value ?? "").Replace("\r", "&#13;");

    private static string RenderNode(JsonElement node)
    {
        var kind = node.TryGetProperty("kind", out var kindElement) ? kindElement.GetString() : "";
        return kind switch
        {
            "text" => System.Net.WebUtility.HtmlEncode(node.GetProperty("content").GetString() ?? ""),
            "stack" or "scroll" => $"<div data-crepus-kind=\"{EscapeAttribute(kind)}\" data-axis=\"{EscapeAttribute(node.GetProperty("axis").GetString() ?? "column")}\">{string.Concat(node.GetProperty("children").EnumerateArray().Select(RenderNode))}</div>",
            "button" => RenderButton(node),
            "link" => RenderLink(node),
            "image" => $"<img src=\"{EscapeAttribute(node.GetProperty("src").GetString() ?? "")}\" alt=\"{EscapeAttribute(node.GetProperty("alt").GetString() ?? "")}\">",
            "slotRotate" => $"<span data-crepus-kind=\"slotRotate\">{System.Net.WebUtility.HtmlEncode(FirstPhrase(node))}</span>",
            "input" => RenderInput(node),
            "picker" => RenderPicker(node),
            _ => ""
        };
    }

    private static string RenderLink(JsonElement node)
    {
        var href = node.TryGetProperty("href", out var hrefElement) && hrefElement.ValueKind == JsonValueKind.String
            ? hrefElement.GetString() ?? "" : "";
        var attributes = SafeLinkHref(href) ? $" href=\"{EscapeAttribute(href)}\"" : "";
        foreach (var key in new[] { "target", "rel" })
        {
            if (node.TryGetProperty(key, out var value) && value.ValueKind == JsonValueKind.String)
                attributes += $" {key}=\"{EscapeAttribute(value.GetString())}\"";
        }
        var children = node.TryGetProperty("children", out var childNodes) && childNodes.ValueKind == JsonValueKind.Array
            ? string.Concat(childNodes.EnumerateArray().Select(RenderNode)) : "";
        return $"<a{attributes}>{children}</a>";
    }

    private static bool SafeLinkHref(string value)
    {
        // Match browser scheme parsing, using ASCII case folding only.
        var start = 0;
        while (start < value.Length && value[start] <= '\u0020') start++;
        var scheme = new System.Text.StringBuilder();
        for (var i = start; i < value.Length; i++)
        {
            var character = value[i];
            if (character is '\t' or '\r' or '\n') continue;
            if (character == ':')
                return scheme.ToString() is not ("javascript" or "vbscript" or "data");
            scheme.Append(character is >= 'A' and <= 'Z' ? (char)(character + ('a' - 'A')) : character);
        }
        return true;
    }

    private static string RenderButton(JsonElement node)
    {
        var label = System.Net.WebUtility.HtmlEncode(node.GetProperty("label").GetString() ?? "");
        if (!node.TryGetProperty("onClick", out var onClick))
        {
            return $"<button>{label}</button>";
        }
        return $"<button data-onclick=\"{EscapeAttribute(onClick.GetString() ?? "")}\">{label}</button>";
    }

    private static readonly HashSet<string> HtmlInputTypes = new(StringComparer.Ordinal)
    {
        "text", "search", "tel", "url", "email", "password", "number", "date",
        "datetime-local", "month", "time", "week", "color", "hidden"
    };

    private static string RenderInput(JsonElement node)
    {
        var bind = node.TryGetProperty("bind", out var bindElement) ? bindElement.GetString() ?? "" : "";
        var placeholder = node.TryGetProperty("placeholder", out var placeholderElement) ? placeholderElement.GetString() ?? "" : "";
        var attributes = $" data-bind=\"{EscapeAttribute(bind)}\" placeholder=\"{EscapeAttribute(placeholder)}\"";
        var hasStyle = node.TryGetProperty("style", out var style) && style.ValueKind == JsonValueKind.Object;
        if (hasStyle && style.TryGetProperty("formName", out var name) && name.ValueKind == JsonValueKind.String)
            attributes += $" name=\"{EscapeAttribute(name.GetString())}\"";
        if (hasStyle && style.TryGetProperty("required", out var required) && required.ValueKind == JsonValueKind.True)
            attributes += " required";
        var value = bind.Length == 0 && hasStyle && style.TryGetProperty("formValue", out var initialValue)
            && initialValue.ValueKind != JsonValueKind.Null ? initialValue.ToString() : null;
        if (node.TryGetProperty("multiline", out var multiline) && multiline.ValueKind == JsonValueKind.True)
        {
            var text = System.Net.WebUtility.HtmlEncode(value ?? "").Replace("\r", "&#13;");
            // HTML discards one initial LF; encode CR to retain it through parsing.
            if (text.StartsWith('\n')) text = "\n" + text;
            return $"<textarea{attributes}>{text}</textarea>";
        }
        var secure = node.TryGetProperty("secure", out var secureElement) && secureElement.ValueKind == JsonValueKind.True;
        var inputType = secure ? "password"
            : hasStyle && style.TryGetProperty("inputType", out var type) && type.ValueKind == JsonValueKind.String ? type.GetString() : null;
        var typeAttribute = inputType is not null && HtmlInputTypes.Contains(inputType) ? $" type=\"{inputType}\"" : "";
        var initial = value is null ? "" : $" value=\"{EscapeAttribute(value)}\"";
        return $"<input{typeAttribute}{attributes}{initial}>";
    }

    private static string RenderPicker(JsonElement node)
    {
        var bind = EscapeAttribute(node.GetProperty("bind").GetString() ?? "");
        var options = node.GetProperty("options").EnumerateArray().Select(option =>
            $"<option value=\"{EscapeAttribute(option.GetProperty("value").GetString() ?? "")}\">{System.Net.WebUtility.HtmlEncode(option.GetProperty("label").GetString() ?? "")}</option>"
        );
        return $"<select data-bind=\"{bind}\">{string.Concat(options)}</select>";
    }

    private static string FirstPhrase(JsonElement node)
    {
        if (!node.TryGetProperty("phrases", out var phrases))
        {
            return "";
        }
        var first = phrases.EnumerateArray().FirstOrDefault();
        return first.ValueKind == JsonValueKind.Undefined ? "" : first.GetString() ?? "";
    }
}
