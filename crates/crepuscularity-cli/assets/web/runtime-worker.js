/** One runtime instance and one FIFO for stateful calls and render transactions. */
export function createRuntimeQueue(loadRuntime) {
  let runtime;
  let pending = Promise.resolve();
  async function initialized() {
    // Cache failure too: initialization is attempted once, never replayed.
    runtime ??= Promise.resolve().then(loadRuntime);
    return runtime;
  }
  async function invoke(exports, name, args) {
    if (typeof name !== "string" || !Object.hasOwn(exports, name) || typeof exports[name] !== "function") {
      throw new Error(`Unknown runtime export: ${String(name)}`);
    }
    if (!Array.isArray(args)) throw new Error("Runtime arguments must be an array");
    return Reflect.apply(exports[name], undefined, args);
  }
  function render(exports, spec) {
    const args = spec.json === true && Array.isArray(spec.args)
      ? spec.args.map((value) => JSON.stringify(value)) : spec.args;
    return invoke(exports, spec.name, args);
  }
  async function execute(request) {
    const exports = await initialized();
    switch (request.kind) {
      case "init":
        return { exports: Object.keys(exports).filter((name) => name !== "default" && typeof exports[name] === "function") };
      case "call":
        return invoke(exports, request.name, request.args);
      case "render":
        return render(exports, request.render);
      case "event":
        await invoke(exports, request.name, [request.eventJson]);
        return request.render == null ? undefined : render(exports, request.render);
      default:
        throw new Error(`Unknown runtime command: ${String(request.kind)}`);
    }
  }
  return function dispatch(request) {
    const result = pending.then(() => execute(request));
    pending = result.catch(() => {});
    return result;
  };
}

if (typeof WorkerGlobalScope !== "undefined" && globalThis instanceof WorkerGlobalScope) {
  const dispatch = createRuntimeQueue(async () => {
    const runtime = await import("./pkg/runtime.js");
    await runtime.default();
    return runtime;
  });
  globalThis.addEventListener("message", ({ data }) => {
    dispatch(data)
      .then((value) => globalThis.postMessage({ id: data.id, value }))
      .catch((error) => {
        let message = "Runtime threw an unprintable value";
        try { message = String(error); } catch (_) {}
        globalThis.postMessage({ id: data?.id, error: message });
      });
  });
}
