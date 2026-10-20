using System.Runtime.InteropServices;
using Crepuscularity.Plugin;

if (args.Length != 1)
    throw new ArgumentException("Usage: Lifetime <absolute ABI library path>");
var library = NativeLibrary.Load(Path.GetFullPath(args[0]));
NativeLibrary.SetDllImportResolver(typeof(CrepusAbiSession).Assembly,
    (name, assembly, searchPath) => name == "crepuscularity_abi" ? library : IntPtr.Zero);

var session = new CrepusAbiSession();
session.SetTemplate("span \"{count}\"");
session.SetContext(new { count = 1 });
session.PatchContext(new { count = 2 });
using (var rendered = session.RenderIr())
    Require(rendered.RootElement.GetProperty("root").GetArrayLength() > 0, "valid render");
using (var dispatched = session.DispatchEvent(new { handler = "accept" }))
    Require(dispatched.RootElement.GetProperty("kind").GetString() == "event", "valid dispatch");
session.Dispose();
session.Dispose();

var operations = new (string Name, Action Run)[] {
    ("SetTemplate", () => session.SetTemplate("span \"after dispose\"")),
    ("SetContext", () => session.SetContext(new { count = 3 })),
    ("PatchContext", () => session.PatchContext(new { count = 4 })),
    ("RenderIr", () => { using var result = session.RenderIr(); }),
    ("DispatchEvent", () => { using var result = session.DispatchEvent(new { handler = "after_dispose" }); }),
    ("SetContext before serialization", () => session.SetContext(new PoisonPayload())),
    ("PatchContext before serialization", () => session.PatchContext(new PoisonPayload())),
    ("DispatchEvent before serialization", () => { using var result = session.DispatchEvent(new PoisonPayload()); }),
};
var failures = new List<string>();
foreach (var (name, run) in operations) {
    try { run(); failures.Add($"{name}: operation reached native code after disposal"); }
    catch (ObjectDisposedException) { }
    catch (Exception error) { failures.Add($"{name}: expected ObjectDisposedException, got {error.GetType().Name}"); }
}
Require(failures.Count == 0, string.Join(Environment.NewLine, failures));
Console.WriteLine("PASS actual C# ABI wrapper: valid lifecycle, idempotent dispose, five disposed guards before serialization");

static void Require(bool condition, string message) {
    if (!condition) throw new InvalidOperationException(message);
}
sealed class PoisonPayload {
    public string Value => throw new InvalidOperationException("JSON serialization ran after disposal");
}
