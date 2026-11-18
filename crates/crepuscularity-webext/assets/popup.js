import { browserApi } from "./browser-shim.js";

// popup.html is pre-rendered at build time by `crepus webext build`.
// Handles storage sync for toggles and view switching for help / crepus views.
// No WASM — popup opens instantly from static HTML.

const viewMain = document.getElementById("view-main");
const viewHelp = document.getElementById("view-help");
const viewCrepus = document.getElementById("view-crepus");

const DEFAULTS = { enabled: true, autoRender: false };
const revisions = { enabled: 0, autoRender: 0 };
const pendingWrites = { enabled: 0, autoRender: 0 };

function showView(which) {
  if (viewMain) viewMain.hidden = which !== "main";
  if (viewHelp) viewHelp.hidden = which !== "help";
  if (viewCrepus) viewCrepus.hidden = which !== "crepus";
}

async function syncState() {
  const requested = { ...revisions };
  const writing = { ...pendingWrites };
  const data = await browserApi.storage.sync
    .get(DEFAULTS)
    .catch(() => DEFAULTS);
  const settings = { ...DEFAULTS, ...data };

  const enabledCb = document.getElementById("enabled");
  const autoRenderCb = document.getElementById("autoRender");
  if (enabledCb && !writing.enabled && requested.enabled === revisions.enabled) {
    enabledCb.checked = settings.enabled;
  }
  if (autoRenderCb && !writing.autoRender && requested.autoRender === revisions.autoRender) {
    autoRenderCb.checked = settings.autoRender;
  }
}

syncState();

document.addEventListener("click", (e) => {
  const btn = e.target.closest("[data-action]");
  if (!btn) return;
  const action = btn.dataset.action;

  if (action === "show-help") {
    showView("help");
  } else if (action === "hide-help") {
    showView("main");
    syncState();
  } else if (action === "show-crepus") {
    showView("crepus");
  } else if (action === "hide-crepus") {
    showView("help");
  } else if (action === "copy-prompt") {
    const pre = document.getElementById("system-prompt");
    if (pre) {
      navigator.clipboard.writeText(pre.textContent).then(() => {
        const icon = btn.querySelector(".material-symbols-outlined");
        if (icon) {
          icon.textContent = "check";
          setTimeout(() => { icon.textContent = "content_copy"; }, 1500);
        }
      }).catch(() => {});
    }
  }
});

document.addEventListener("change", async (e) => {
  const actionEl = e.target.closest("input[data-action]");
  if (!actionEl) return;

  const action = actionEl.dataset.action;
  if (action === "set-enabled") {
    revisions.enabled++;
    pendingWrites.enabled++;
    await browserApi.storage.sync.set({ enabled: actionEl.checked }).catch(() => {});
    pendingWrites.enabled--;
  } else if (action === "set-auto-render") {
    revisions.autoRender++;
    pendingWrites.autoRender++;
    await browserApi.storage.sync.set({ autoRender: actionEl.checked }).catch(() => {});
    pendingWrites.autoRender--;
  }
});
