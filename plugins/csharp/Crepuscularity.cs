using System.Diagnostics;
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
        var template = await File.ReadAllTextAsync(path);
        var payload = JsonSerializer.Serialize(new { template, context = context ?? new { } });
        var bin = Environment.GetEnvironmentVariable("CREPUS_BIN") ?? "crepus";
        using var proc = new Process();
        proc.StartInfo = new ProcessStartInfo(bin, "native ir --stdin-json")
        {
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };
        proc.Start();
        await proc.StandardInput.WriteAsync(payload);
        proc.StandardInput.Close();
        var stdout = await proc.StandardOutput.ReadToEndAsync();
        var stderr = await proc.StandardError.ReadToEndAsync();
        await proc.WaitForExitAsync();
        if (proc.ExitCode != 0)
        {
            throw new InvalidOperationException(stderr);
        }
        return JsonSerializer.Deserialize<ViewIr>(stdout) ?? throw new InvalidOperationException("empty IR");
    }

    public static async Task<string> RenderHtmlAsync(string path, object? context = null)
    {
        var ir = await RenderIrAsync(path, context);
        return string.Concat(ir.Root.EnumerateArray().Select(RenderNode));
    }

    private static string RenderNode(JsonElement node)
    {
        var kind = node.TryGetProperty("kind", out var kindElement) ? kindElement.GetString() : "";
        return kind switch
        {
            "text" => System.Net.WebUtility.HtmlEncode(node.GetProperty("content").GetString() ?? ""),
            "stack" or "scroll" => $"<div data-crepus-kind=\"{System.Net.WebUtility.HtmlEncode(kind)}\" data-axis=\"{System.Net.WebUtility.HtmlEncode(node.GetProperty("axis").GetString() ?? "column")}\">{string.Concat(node.GetProperty("children").EnumerateArray().Select(RenderNode))}</div>",
            "button" => RenderButton(node),
            "image" => $"<img src=\"{System.Net.WebUtility.HtmlEncode(node.GetProperty("src").GetString() ?? "")}\" alt=\"{System.Net.WebUtility.HtmlEncode(node.GetProperty("alt").GetString() ?? "")}\">",
            "slotRotate" => $"<span data-crepus-kind=\"slotRotate\">{System.Net.WebUtility.HtmlEncode(FirstPhrase(node))}</span>",
            "input" => RenderInput(node),
            "picker" => RenderPicker(node),
            _ => ""
        };
    }

    private static string RenderButton(JsonElement node)
    {
        var label = System.Net.WebUtility.HtmlEncode(node.GetProperty("label").GetString() ?? "");
        if (!node.TryGetProperty("onClick", out var onClick))
        {
            return $"<button>{label}</button>";
        }
        return $"<button data-onclick=\"{System.Net.WebUtility.HtmlEncode(onClick.GetString() ?? "")}\">{label}</button>";
    }

    private static string RenderInput(JsonElement node)
    {
        var bind = System.Net.WebUtility.HtmlEncode(node.GetProperty("bind").GetString() ?? "");
        var placeholder = System.Net.WebUtility.HtmlEncode(node.GetProperty("placeholder").GetString() ?? "");
        if (node.TryGetProperty("multiline", out var multiline) && multiline.ValueKind == JsonValueKind.True)
        {
            return $"<textarea data-bind=\"{bind}\" placeholder=\"{placeholder}\"></textarea>";
        }
        return $"<input data-bind=\"{bind}\" placeholder=\"{placeholder}\">";
    }

    private static string RenderPicker(JsonElement node)
    {
        var bind = System.Net.WebUtility.HtmlEncode(node.GetProperty("bind").GetString() ?? "");
        var options = node.GetProperty("options").EnumerateArray().Select(option =>
            $"<option value=\"{System.Net.WebUtility.HtmlEncode(option.GetProperty("value").GetString() ?? "")}\">{System.Net.WebUtility.HtmlEncode(option.GetProperty("label").GetString() ?? "")}</option>"
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
