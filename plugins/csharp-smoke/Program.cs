using Crepuscularity.Plugin;
using Crepus = Crepuscularity.Plugin.Crepuscularity;

if (args.Length > 0 && args[0] == "--transport")
{
    await TransportChecks.Run();
    return;
}

if (args.Length > 0 && args[0] == "--input-html")
{
    await InputHtmlChecks.Run(args[1]);
    return;
}

if (args.Length > 0 && args[0] == "--link-html")
{
    await LinkHtmlChecks.Run();
    return;
}

if (args.Length > 0 && args[0] == "--abi-nul")
{
    AbiNulChecks.Run();
    return;
}

var context = new Dictionary<string, object?> { ["name"] = "Ada" };
var ir = await Crepus.RenderIrAsync(args[0], context);
if (ir.Version != 7 || !ir.Root.ToString().Contains("Hello Ada"))
    throw new InvalidOperationException($"unexpected IR: {ir}");
var frontend = await Crepus.RenderIrAsync(Path.Combine(Path.GetDirectoryName(Path.GetFullPath(args[0]))!, "frontend.svelte"), context);
if (!frontend.Root.ToString().Contains("Frontend content") || frontend.Root.ToString().Contains("ignored"))
    throw new InvalidOperationException($"frontend output: {frontend}");

var directory = Directory.CreateTempSubdirectory("crepus-csharp-include-");
try
{
    var source = Path.Combine(directory.FullName, "index.crepus");
    await File.WriteAllTextAsync(source, "include part.crepus name={name}\n");
    await File.WriteAllTextAsync(Path.Combine(directory.FullName, "part.crepus"), "span\n  \"Included {name}\"\n");
    var included = await Crepus.RenderIrAsync(source, context);
    if (!included.Root.ToString().Contains("Included Ada"))
        throw new InvalidOperationException($"include output: {included}");
    if (!OperatingSystem.IsWindows())
    {
        var binary = Path.Combine(directory.FullName, "crepus");
        await File.WriteAllTextAsync(binary,
            "#!/usr/bin/env python3\nimport sys\nsys.stdin.read()\nsys.stderr.write('x' * 262144)\nsys.stderr.flush()\nprint('{\"version\":7,\"root\":[]}')\n");
        File.SetUnixFileMode(binary, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);
        var original = Environment.GetEnvironmentVariable("CREPUS_BIN");
        Environment.SetEnvironmentVariable("CREPUS_BIN", binary);
        try
        {
            var diagnosticIr = await Crepus.RenderIrAsync(source).WaitAsync(TimeSpan.FromSeconds(5));
            if (diagnosticIr.Version != 7)
                throw new InvalidOperationException($"diagnostic output: {diagnosticIr}");
        }
        finally
        {
            Environment.SetEnvironmentVariable("CREPUS_BIN", original);
        }
    }
}
finally
{
    directory.Delete(recursive: true);
}
