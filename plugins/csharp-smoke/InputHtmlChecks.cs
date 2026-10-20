using System.Text.Json;
using Crepuscularity.Plugin;
using Crepus = Crepuscularity.Plugin.Crepuscularity;

internal static class InputHtmlChecks
{
    public static async Task Run(string fixtureDirectory)
    {
        if (OperatingSystem.IsWindows())
            throw new PlatformNotSupportedException("Input HTML fixture transport uses a Unix CLI stub.");
        var directory = Directory.CreateTempSubdirectory("crepus-csharp-input-");
        var originalBinary = Environment.GetEnvironmentVariable("CREPUS_BIN");
        var originalFixture = Environment.GetEnvironmentVariable("CREPUS_INPUT_HTML_FIXTURE");
        try
        {
            // Labelled CLI transport stub: production public render/session APIs
            // consume the shared IR without depending on the Rust parser.
            var binary = Path.Combine(directory.FullName, "crepus");
            await File.WriteAllTextAsync(binary,
                "#!/usr/bin/env python3\nimport os,sys\nsys.stdin.read()\nwith open(os.environ['CREPUS_INPUT_HTML_FIXTURE']) as f: sys.stdout.write(f.read())\n");
            File.SetUnixFileMode(binary, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
            Environment.SetEnvironmentVariable("CREPUS_BIN", binary);
            var source = Path.Combine(directory.FullName, "input.crepus");
            await File.WriteAllTextAsync(source, "input\n");
            foreach (var fixtureName in new[] { "input-defaults.view-ir.json", "input-metadata.view-ir.json" })
            {
                var fixturePath = Path.GetFullPath(Path.Combine(fixtureDirectory, fixtureName));
                using var fixture = JsonDocument.Parse(await File.ReadAllTextAsync(fixturePath));
                Environment.SetEnvironmentVariable("CREPUS_INPUT_HTML_FIXTURE", fixturePath);
                var expected = fixture.RootElement.GetProperty("expectedHtml").GetString();
                var actual = await Crepus.RenderHtmlAsync(source);
                if (actual != expected)
                    throw new InvalidOperationException($"{fixtureName} HTML mismatch:\nexpected: {expected}\nactual: {actual}");
                if (fixtureName == "input-defaults.view-ir.json" &&
                    Environment.GetEnvironmentVariable("CREPUS_HTML_OUTPUT") is { Length: > 0 } output)
                    await File.WriteAllTextAsync(output, actual);
                var session = new ViewSession(source);
                if (await session.RenderHtmlAsync() != expected)
                    throw new InvalidOperationException($"{fixtureName} session HTML mismatch");
            }
            Environment.SetEnvironmentVariable("CREPUS_BIN", originalBinary);
            await File.WriteAllTextAsync(source, "input type=hidden name={field} placeholder={hint} value={value}\n");
            var context = new Dictionary<string, object> {
                ["field"] = "field\rname", ["hint"] = "hint\r\nline", ["value"] = "value\rdata"
            };
            var real = await Crepus.RenderHtmlAsync(source, context);
            var realExpected = "<input type=\"hidden\" data-bind=\"\" placeholder=\"hint&#13;\nline\" name=\"field&#13;name\" value=\"value&#13;data\">";
            if (real != realExpected)
                throw new InvalidOperationException($"Actual CLI attribute HTML mismatch:\nexpected: {realExpected}\nactual: {real}");
            if (Environment.GetEnvironmentVariable("CREPUS_HTML_OUTPUT") is { Length: > 0 } realOutput)
                await File.WriteAllTextAsync(realOutput + ".real.html", real);
            Console.WriteLine("C# input defaults + metadata public HTML/session and actual CLI attributes PASS");
        }
        finally
        {
            Environment.SetEnvironmentVariable("CREPUS_BIN", originalBinary);
            Environment.SetEnvironmentVariable("CREPUS_INPUT_HTML_FIXTURE", originalFixture);
            directory.Delete(recursive: true);
        }
    }
}
