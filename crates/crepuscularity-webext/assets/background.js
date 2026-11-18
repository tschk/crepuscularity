import { browserApi } from "./browser-shim.js";
import init, * as runtimeModule from "../vendor/runtime.js";

const DEFAULT_SETTINGS = {
  enabled: true,
  autoRender: false
};

function storageFailure(error) {
  let message = "Storage operation failed";
  try { message = String(error?.message ?? error); } catch (_) {
    try { message = String(error); } catch (_) {}
  }
  return { ok: false, error: message };
}

const api = globalThis.browser ?? globalThis.chrome;
if (!api?.runtime) {
  throw new Error("browser runtime API is unavailable");
}

const runtimeReady = fetch("../vendor/runtime_bg.wasm")
  .then((response) => response.arrayBuffer())
  .then((wasmBytes) => init({ module_or_path: wasmBytes }))
  .then(() => runtimeModule)
  .catch((error) => {
    console.error("background WASM init failed", error);
    return null;
  });

api.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type === "settings:get") {
    browserApi.storage.sync
      .get(DEFAULT_SETTINGS)
      .then((settings) => sendResponse({
        ok: true,
        settings: { ...DEFAULT_SETTINGS, ...settings }
      }))
      .catch((error) => sendResponse(storageFailure(error)));
    return true;
  }

  if (message?.type === "settings:set") {
    const nextSettings = { ...message.settings };
    browserApi.storage.sync
      .set(nextSettings)
      .then(() => browserApi.storage.sync.get(DEFAULT_SETTINGS))
      .then((settings) => sendResponse({
        ok: true,
        settings: { ...DEFAULT_SETTINGS, ...settings }
      }))
      .catch((error) => sendResponse(storageFailure(error)));
    return true;
  }

  runtimeReady
    .then((runtime) => {
      if (runtime && typeof runtime.handle_background_message === "function") {
        return runtime.handle_background_message(message);
      }
      return { ok: false, error: "unknown message type" };
    })
    .then((response) => sendResponse(response))
    .catch((error) => {
      let message = "Runtime threw an unprintable value";
      try { message = String(error); } catch (_) {}
      sendResponse({ ok: false, error: message });
    });
  return true;
});

api.runtime.onInstalled.addListener(() => {
  runtimeReady
    .then((runtime) => {
      if (runtime && typeof runtime.settings_seed === "function") {
        return runtime.settings_seed();
      }
    })
    .catch((error) => {
      console.error("settings_seed failed on install", error);
    });
});

api.storage.onChanged.addListener((_changes, area) => {
  if (area !== "sync") return;
  runtimeReady
    .then((runtime) => {
      if (runtime && typeof runtime.settings_seed === "function") {
        return runtime.settings_seed();
      }
    })
    .catch((error) => {
      console.error("settings_seed failed on storage change", error);
    });
});
