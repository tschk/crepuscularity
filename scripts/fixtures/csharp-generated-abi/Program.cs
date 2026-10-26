using System.Runtime.InteropServices;
using System.Text.Json;
using Abi = EquilibriumImports;

if (args.Length != 1)
    throw new ArgumentException("Usage: GeneratedAbi <absolute ABI library path>");
var library = NativeLibrary.Load(Path.GetFullPath(args[0]));
NativeLibrary.SetDllImportResolver(typeof(Abi).Assembly,
    (name, assembly, searchPath) => name == "crepuscularity_abi" ? library : IntPtr.Zero);

var session = Abi.crepus_session_new();
Require(session != IntPtr.Zero, "session allocation");
var calls = 0;
var callbackError = "";
var userdata = new IntPtr(73);
EventCallback callback = (eventJson, receivedUserdata) => {
    // Do not throw across the unmanaged callback boundary.
    var raw = Marshal.PtrToStringUTF8(eventJson) ?? "";
    if (receivedUserdata != userdata || !raw.Contains("bind:count:4"))
        callbackError = "callback payload or userdata changed";
    calls++;
};
try {
    using (var source = new Utf8("input bind=count\nspan \"Count {count} 雪\""))
        Ok(Abi.crepus_session_set_template_string(session, source.Pointer, IntPtr.Zero));
    using (var context = new Utf8("{\"count\":\"1\"}"))
        Ok(Abi.crepus_session_set_context_json(session, context.Pointer));
    using (var patch = new Utf8("{\"count\":\"3\"}"))
        Ok(Abi.crepus_session_apply_context_patch_json(session, patch.Pointer));
    Require(Take(Abi.crepus_session_render_ir_json(session)).Contains("Count 3 雪"), "context patch/render UTF-8");

    Ok(Abi.crepus_session_set_event_callback(session, Marshal.GetFunctionPointerForDelegate(callback), userdata));
    using (var eventJson = new Utf8("{\"handler\":\"bind:count:4\"}")) {
        Require(Take(Abi.crepus_session_dispatch_event_json(session, eventJson.Pointer)).Contains("Count 4 雪"), "event state update");
        Require(calls == 1 && callbackError == "", "rooted callback and userdata");
        Ok(Abi.crepus_session_set_event_callback(session, IntPtr.Zero, IntPtr.Zero));
        Take(Abi.crepus_session_dispatch_event_json(session, eventJson.Pointer));
        Require(calls == 1, "cleared callback must not run");
    }

    using (var invalid = new Utf8("{"))
        Require(Abi.crepus_session_set_context_json(session, invalid.Pointer) == -1, "signed error status");
    Require(Take(Abi.crepus_session_take_last_error(session)).Length > 0, "session error");
    Require(Abi.crepus_session_take_last_error(session) == IntPtr.Zero, "session error consumed");

    using (var files = new Utf8("{\"entry\":\"app.crepus\",\"files\":{\"app.crepus\":\"span \\\"Virtual file 雪\\\"\"}}"))
        Ok(Abi.crepus_session_set_files_json(session, files.Pointer));
    Require(Take(Abi.crepus_session_render_ir_json(session)).Contains("Virtual file 雪"), "virtual files render");

    using (var components = new Utf8("--- First\nspan \"Wrong first\"\n--- Second\nspan \"Selected second 雪\""))
        Ok(Abi.crepus_session_set_template_string(session, components.Pointer, IntPtr.Zero));
    using (var component = new Utf8("Second"))
        Ok(Abi.crepus_session_set_component(session, component.Pointer));
    var selected = Take(Abi.crepus_session_render_ir_json(session));
    Require(selected.Contains("Selected second 雪") && !selected.Contains("Wrong first"), "second named component selected");
    Ok(Abi.crepus_session_set_component(session, IntPtr.Zero));
    using (var source = new Utf8("span \"Reset component\""))
        Ok(Abi.crepus_session_set_template_string(session, source.Pointer, IntPtr.Zero));
    Require(Take(Abi.crepus_session_render_ir_json(session)).Contains("Reset component"), "component cleared");
} finally {
    // KeepAlive spans all native calls, including unregister and exceptional cleanup.
    Abi.crepus_session_set_event_callback(session, IntPtr.Zero, IntPtr.Zero);
    Abi.crepus_session_free(session);
    GC.KeepAlive(callback);
}
using (var context = new Utf8("{}"))
    Require(Abi.crepus_session_set_context_json(IntPtr.Zero, context.Pointer) == -1, "null session error");
Require(Take(Abi.crepus_last_error()).Length > 0, "global error");
Require(Abi.crepus_last_error() == IntPtr.Zero, "global error consumed");
Console.WriteLine("PASS generated C# ABI: all 13 exports, UTF-8 ownership, context/files/components, rooted nullable Cdecl callback/userdata and signed errors");

static void Require(bool condition, string message) {
    if (!condition) throw new InvalidOperationException(message);
}
static void Ok(int status) => Require(status == 0, $"ABI status {status}");
static string Take(IntPtr pointer) {
    Require(pointer != IntPtr.Zero, "expected owned ABI string");
    try { return Marshal.PtrToStringUTF8(pointer) ?? ""; }
    finally { Abi.crepus_string_free(pointer); }
}
[UnmanagedFunctionPointer(CallingConvention.Cdecl)]
delegate void EventCallback(IntPtr eventJson, IntPtr userdata);
sealed class Utf8 : IDisposable {
    public IntPtr Pointer { get; }
    public Utf8(string value) => Pointer = Marshal.StringToCoTaskMemUTF8(value);
    public void Dispose() => Marshal.FreeCoTaskMem(Pointer);
}
