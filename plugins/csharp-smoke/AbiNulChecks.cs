using Crepuscularity.Plugin;
using System.Runtime.InteropServices;

internal static class AbiNulChecks
{
    public static void Run()
    {
        if (Environment.GetEnvironmentVariable("CREPUS_ABI_LIB") is { Length: > 0 } library)
            NativeLibrary.SetDllImportResolver(typeof(CrepusAbiSession).Assembly,
                (name, _, _) => name == "crepuscularity_abi" ? NativeLibrary.Load(library) : IntPtr.Zero);
        // Real managed wrapper + native ABI: no transport/native collaborator stubs.
        var root = Directory.CreateTempSubdirectory("crepus-csharp-abi-nul-");
        try
        {
            var directory = Directory.CreateDirectory(Path.Combine(root.FullName, "雪"));
            File.WriteAllText(Path.Combine(directory.FullName, "part.crepus"), "span \"Unicode 雪 🦋\"\n");
            using var session = new CrepusAbiSession();
            session.SetTemplate("include part.crepus", directory.FullName);
            using var original = session.RenderIr();
            var expected = original.RootElement.GetRawText();
            var content = original.RootElement.GetProperty("root")[0].GetProperty("content").GetString();
            if (content != "Unicode 雪 🦋")
                throw new InvalidOperationException($"Unicode template/baseDir did not survive ABI: {expected}");

            void Reject(Action action, string parameter)
            {
                try
                {
                    action();
                    throw new InvalidOperationException($"NUL in {parameter} was accepted");
                }
                catch (ArgumentException error) when (error.ParamName == parameter) { }
                using var unchanged = session.RenderIr();
                if (unchanged.RootElement.GetRawText() != expected)
                    throw new InvalidOperationException($"Rejected {parameter} replaced the previous template");
            }

            foreach (var source in new[] { "\0span \"after\"", "span \"before\"\0\nspan \"after\"", "span \"after\"\0" })
                Reject(() => session.SetTemplate(source), "template");
            foreach (var path in new[] { "\0" + directory.FullName, directory.FullName + "\0ignored" })
                Reject(() => session.SetTemplate("span \"replacement\"", path), "baseDir");

            session.SetTemplate("span \"Still usable 雪\"");
            using var replacement = session.RenderIr();
            if (replacement.RootElement.GetProperty("root")[0].GetProperty("content").GetString() != "Still usable 雪")
                throw new InvalidOperationException("Session unusable after rejected arguments");
            Console.WriteLine("C# ABI NUL rejection/template preservation/Unicode PASS");
        }
        finally
        {
            root.Delete(recursive: true);
        }
    }
}
