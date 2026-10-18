using System.Text.Json;
using Crepuscularity.Plugin;
using Crepus = Crepuscularity.Plugin.Crepuscularity;

internal static class LinkHtmlChecks
{
    public static async Task Run()
    {
        if (OperatingSystem.IsWindows())
            throw new PlatformNotSupportedException("Link HTML fixture transport uses a Unix CLI stub.");
        var directory = Directory.CreateTempSubdirectory("crepus-csharp-link-");
        var previous = Environment.GetEnvironmentVariable("CREPUS_BIN");
        try
        {
            object Text(string value) => new { kind = "text", content = value };
            var root = new List<object>
            {
                new { kind = "link", href = "/雪?q=\"<&", hrefBind = "scope.next", target = "\"_blank", rel = "next & \"safe\"", children = new object[] {
                    Text("Help 雪 <&\""),
                    new { kind = "stack", axis = "column", children = new object[] {
                        new { kind = "link", href = "/nested", children = new[] { Text("Nested") } },
                        new { kind = "button", label = "Save", onClick = "save\"&" }
                    } }
                } },
                new { kind = "link", href = "", target = "", rel = (string?)null, children = new[] { Text("Empty") } },
                new { kind = "link", href = "/plain", children = Array.Empty<object>() }
            };
            foreach (var href in new[] { "javascript:alert(1)", "\0 \tJaVa\nScRiPt:bad()", "VB\rScript:bad", "\u001fdata:text/html,bad", "&#106;avascript:literal", "/relative?q=1&x=2", "https://example.test/雪", "mailto:user@example.test", "ſavascript:literal" })
                root.Add(new { kind = "link", href, children = new[] { Text("safe") } });
            var fixture = Path.Combine(directory.FullName, "links.json");
            await File.WriteAllTextAsync(fixture, JsonSerializer.Serialize(new { version = 7, root }));
            var binary = Path.Combine(directory.FullName, "crepus");
            // Labelled CLI transport stub isolates public HTML/session conversion.
            await File.WriteAllTextAsync(binary, "#!/usr/bin/env python3\nimport sys\nsys.stdin.read()\nwith open(" + JsonSerializer.Serialize(fixture) + ") as f: sys.stdout.write(f.read())\n");
            File.SetUnixFileMode(binary, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
            Environment.SetEnvironmentVariable("CREPUS_BIN", binary);
            var source = Path.Combine(directory.FullName, "links.crepus");
            await File.WriteAllTextAsync(source, "a href=\"/unused\"\n");
            var expected = "<a href=\"/雪?q=&quot;&lt;&amp;\" target=\"&quot;_blank\" rel=\"next &amp; &quot;safe&quot;\">Help 雪 &lt;&amp;&quot;<div data-crepus-kind=\"stack\" data-axis=\"column\"><a href=\"/nested\">Nested</a><button data-onclick=\"save&quot;&amp;\">Save</button></div></a>"
                + "<a href=\"\" target=\"\">Empty</a><a href=\"/plain\"></a>"
                + string.Concat(Enumerable.Repeat("<a>safe</a>", 4))
                + "<a href=\"&amp;#106;avascript:literal\">safe</a><a href=\"/relative?q=1&amp;x=2\">safe</a><a href=\"https://example.test/雪\">safe</a><a href=\"mailto:user@example.test\">safe</a><a href=\"ſavascript:literal\">safe</a>";
            var actual = await Crepus.RenderHtmlAsync(source);
            if (actual != expected) throw new InvalidOperationException($"Link HTML mismatch:\nexpected: {expected}\nactual: {actual}");
            if (await new ViewSession(source).RenderHtmlAsync() != expected)
                throw new InvalidOperationException("Link session HTML mismatch");
            if (Environment.GetEnvironmentVariable("CREPUS_HTML_OUTPUT") is { } output)
                await File.WriteAllTextAsync(output, actual);
            Console.WriteLine("C# Link public HTML/session PASS");
        }
        finally
        {
            Environment.SetEnvironmentVariable("CREPUS_BIN", previous);
            directory.Delete(recursive: true);
        }
    }
}
