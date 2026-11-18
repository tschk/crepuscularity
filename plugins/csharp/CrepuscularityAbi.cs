using System.Runtime.InteropServices;
using System.Text.Json;

namespace Crepuscularity.Plugin;

public sealed class CrepusAbiSession : IDisposable
{
    private readonly IntPtr _session;
    private bool _disposed;

    public CrepusAbiSession()
    {
        _session = Native.crepus_session_new();
        if (_session == IntPtr.Zero)
        {
            throw new InvalidOperationException("crepus_session_new failed");
        }
    }

    public void SetTemplate(string template, string? baseDir = null)
    {
        Check(Native.crepus_session_set_template_string(_session, template, baseDir));
    }

    public void SetContext(object context)
    {
        Check(Native.crepus_session_set_context_json(_session, JsonSerializer.Serialize(context)));
    }

    public void PatchContext(object context)
    {
        Check(Native.crepus_session_apply_context_patch_json(_session, JsonSerializer.Serialize(context)));
    }

    public JsonDocument RenderIr()
    {
        return TakeJson(Native.crepus_session_render_ir_json(_session));
    }

    public JsonDocument DispatchEvent(object evt)
    {
        return TakeJson(Native.crepus_session_dispatch_event_json(_session, JsonSerializer.Serialize(evt)));
    }

    public void Dispose()
    {
        if (!_disposed)
        {
            Native.crepus_session_free(_session);
            _disposed = true;
        }
    }

    private void Check(int code)
    {
        if (code != 0)
        {
            throw new InvalidOperationException(TakeError());
        }
    }

    private JsonDocument TakeJson(IntPtr ptr)
    {
        if (ptr == IntPtr.Zero)
        {
            throw new InvalidOperationException(TakeError());
        }
        var raw = Marshal.PtrToStringUTF8(ptr) ?? "{}";
        Native.crepus_string_free(ptr);
        return JsonDocument.Parse(raw);
    }

    private string TakeError()
    {
        var ptr = Native.crepus_session_take_last_error(_session);
        if (ptr == IntPtr.Zero)
        {
            return "crepuscularity ABI call failed";
        }
        var error = Marshal.PtrToStringUTF8(ptr) ?? "crepuscularity ABI call failed";
        Native.crepus_string_free(ptr);
        return error;
    }

    private static class Native
    {
        private const string Library = "crepuscularity_abi";

        [DllImport(Library)]
        internal static extern IntPtr crepus_session_new();

        [DllImport(Library)]
        internal static extern void crepus_session_free(IntPtr session);

        [DllImport(Library)]
        internal static extern int crepus_session_set_template_string(
            IntPtr session,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string template,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string? baseDir
        );

        [DllImport(Library)]
        internal static extern int crepus_session_set_context_json(
            IntPtr session,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string contextJson
        );

        [DllImport(Library)]
        internal static extern int crepus_session_apply_context_patch_json(
            IntPtr session,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string contextJson
        );

        [DllImport(Library)]
        internal static extern IntPtr crepus_session_render_ir_json(IntPtr session);

        [DllImport(Library)]
        internal static extern IntPtr crepus_session_dispatch_event_json(
            IntPtr session,
            [MarshalAs(UnmanagedType.LPUTF8Str)] string eventJson
        );

        [DllImport(Library)]
        internal static extern IntPtr crepus_session_take_last_error(IntPtr session);

        [DllImport(Library)]
        internal static extern void crepus_string_free(IntPtr ptr);
    }
}
