using System.Text;
using Crepus = Crepuscularity.Plugin.Crepuscularity;

internal static class TransportChecks
{
    public static async Task Run()
    {
        if (OperatingSystem.IsWindows())
        {
            Console.WriteLine("Skipping transport byte fixture: Unix executable required.");
            return;
        }
        var directory = Directory.CreateTempSubdirectory("crepus-csharp-transport-");
        var previousBin = Environment.GetEnvironmentVariable("CREPUS_BIN");
        var previousMode = Environment.GetEnvironmentVariable("CREPUS_RESPONSE_MODE");
        try
        {
            var source = Path.Combine(directory.FullName, "input.crepus");
            await File.WriteAllTextAsync(source, "span \"Source\"");
            var binary = Path.Combine(directory.FullName, "crepus");
            // Labelled CLI fixture emits exact bytes, independently of the Rust parser.
            await File.WriteAllTextAsync(binary, """
                #!/usr/bin/env python3
                import json,os,sys,threading,time
                open(os.path.join(os.path.dirname(__file__), "called"), "wb").close()
                mode=os.environ['CREPUS_RESPONSE_MODE']
                if mode.startswith('large'):
                    threading.Thread(target=lambda:(time.sleep(5),os._exit(88)),daemon=True).start()
                    if mode == 'large_failure':
                        sys.stderr.buffer.write('雪 early diagnostic\n'.encode('utf-8')); sys.stderr.buffer.flush(); sys.exit(9)
                    if mode == 'large_unread':
                        sys.stdout.buffer.write(b'{"version":7,"root":[]}'); sys.stdout.buffer.flush(); sys.exit(0)
                    sys.stdout.buffer.write(b' ' * 262144); sys.stdout.buffer.flush()
                    payload=json.loads(sys.stdin.buffer.read().decode('utf-8'))
                    assert payload['template'] == 'span "Source"'
                    assert payload['context']['blob'] == '雪' * 400000
                    sys.stdout.buffer.write(json.dumps({'version':7,'root':[{'kind':'text','content':'Exact payload 雪'}]},ensure_ascii=False).encode('utf-8'))
                    sys.exit(0)
                sys.stdin.buffer.read()
                if mode == 'failure':
                    sys.stdout.buffer.write(b'\xff')
                    sys.stderr.buffer.write('雪 diagnostic\n'.encode('utf-8'))
                    sys.exit(9)
                value={'ff':b'\xff','sequence':b'\xc3(','bom_ff':b'\xff'}.get(mode,'雪 🦋'.encode('utf-8'))
                if mode in ('bom', 'bom_ff'): sys.stdout.buffer.write(b'\xef\xbb\xbf')
                sys.stdout.buffer.write(b'{"version":7,"root":[{"kind":"text","content":"'+value+b'"}]}')
                """ + "\n");
            File.SetUnixFileMode(binary, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
            Environment.SetEnvironmentVariable("CREPUS_BIN", binary);
            var failures = new List<string>();
            foreach (var mode in new[] { "valid", "bom", "failure", "ff", "sequence", "bom_ff" })
            foreach (var html in new[] { false, true })
            {
                Environment.SetEnvironmentVariable("CREPUS_RESPONSE_MODE", mode);
                try
                {
                    var output = html ? await Crepus.RenderHtmlAsync(source)
                        : (await Crepus.RenderIrAsync(source)).Root[0].GetProperty("content").GetString();
                    if (mode is not ("valid" or "bom"))
                        failures.Add($"{mode} html={html}: malformed response accepted");
                    else if ((html ? System.Net.WebUtility.HtmlDecode(output) : output) != "雪 🦋")
                        failures.Add($"{mode} html={html}: Unicode changed: {output}");
                }
                catch (DecoderFallbackException) when (mode is "ff" or "sequence" or "bom_ff") { }
                catch (InvalidOperationException error) when (mode == "failure" && error.Message == "雪 diagnostic\n") { }
                catch (Exception error) { failures.Add($"{mode} html={html}: {error}"); }
            }
            Environment.SetEnvironmentVariable("CREPUS_RESPONSE_MODE", "valid");
            var marker = Path.Combine(directory.FullName, "called");
            await File.WriteAllBytesAsync(source, new byte[] { 0xef, 0xbb, 0xbf }.Concat(Encoding.UTF8.GetBytes("span \"雪\"")).ToArray());
            foreach (var html in new[] { false, true })
            {
                File.Delete(marker);
                try
                {
                    if (html) _ = await Crepus.RenderHtmlAsync(source);
                    else _ = await Crepus.RenderIrAsync(source);
                    if (!File.Exists(marker)) failures.Add($"valid BOM source html={html}: CLI was not called");
                }
                catch (Exception error) { failures.Add($"valid BOM source html={html}: {error}"); }
            }
            foreach (var invalid in new[] { new byte[] { 0xff }, new byte[] { 0xc3, 0x28 }, new byte[] { 0xef, 0xbb, 0xbf, 0xff } })
            foreach (var html in new[] { false, true })
            {
                File.Delete(marker);
                await File.WriteAllBytesAsync(source, invalid);
                try
                {
                    if (html) _ = await Crepus.RenderHtmlAsync(source);
                    else _ = await Crepus.RenderIrAsync(source);
                    failures.Add($"source {Convert.ToHexString(invalid)} html={html}: malformed UTF-8 accepted");
                }
                catch (DecoderFallbackException) { }
                catch (Exception error) { failures.Add($"source html={html}: wrong error {error}"); }
                if (File.Exists(marker))
                    failures.Add($"source {Convert.ToHexString(invalid)} html={html}: CLI spawned before rejection");
            }
            await File.WriteAllTextAsync(source, "span \"Source\"");
            var largeContext = new Dictionary<string, object?> { ["blob"] = new string('雪', 400_000) };
            foreach (var mode in new[] { "large", "large_failure", "large_unread" })
            foreach (var html in new[] { false, true })
            {
                Environment.SetEnvironmentVariable("CREPUS_RESPONSE_MODE", mode);
                try
                {
                    var output = html ? await Crepus.RenderHtmlAsync(source, largeContext)
                        : (await Crepus.RenderIrAsync(source, largeContext)).Root[0].GetProperty("content").GetString();
                    if (mode != "large") failures.Add($"{mode} html={html}: incomplete write accepted");
                    else if (output != "Exact payload 雪") failures.Add($"{mode} html={html}: response changed");
                }
                catch (IOException) when (mode == "large_unread") { }
                catch (InvalidOperationException error) when (mode == "large_failure" && error.Message == "雪 early diagnostic\n") { }
                catch (Exception error) { failures.Add($"{mode} html={html}: {error}"); }
            }
            if (failures.Count != 0) throw new InvalidOperationException(string.Join("\n", failures));
            Console.WriteLine("C# transport UTF-8 and diagnostic ordering PASS");
        }
        finally
        {
            Environment.SetEnvironmentVariable("CREPUS_BIN", previousBin);
            Environment.SetEnvironmentVariable("CREPUS_RESPONSE_MODE", previousMode);
            directory.Delete(recursive: true);
        }
    }
}
