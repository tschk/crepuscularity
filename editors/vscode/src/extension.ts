import { resolveServerExecutable } from "./server-executable";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: { instance: LanguageClient; ready: Promise<void> } | undefined;
let generation = 0;
let active = false;
let stopping: Promise<void> = Promise.resolve();

function stopClient(): Promise<void> {
  const current = client;
  client = undefined;
  if (current) {
    const pending = stopping.then(async () => {
      // LanguageClient rejects stop() while start() is still in progress.
      try { await current.ready; } catch { return; }
      try { await current.instance.stop(); }
      catch (error) {
        // Retain the handle so a later restart can retry shutdown first.
        if (!client) client = current;
        throw error;
      }
    });
    // Report this attempt's failure, without poisoning future shutdown attempts.
    stopping = pending.catch(() => {});
    return pending;
  }
  return stopping;
}

function reportStartError(error: unknown, token: number): void {
  if (active && token === generation) {
    void vscode.window.showErrorMessage(`Crepus language server failed to start: ${String(error)}`);
  }
}

async function startClient(token: number): Promise<void> {
  try {
    const folder = vscode.workspace.workspaceFolders?.[0];
    const cfg = vscode.workspace.getConfiguration("crepus");
    const command = await resolveServerExecutable(folder?.uri.fsPath, cfg.get("languageServerPath", ""));
    if (!active || token !== generation) return;
    const serverOptions: ServerOptions = {
      run: { command, args: ["--stdio"], transport: TransportKind.stdio },
      debug: { command, args: ["--stdio"], transport: TransportKind.stdio },
    };
    const clientOptions: LanguageClientOptions = {
      documentSelector: [{ scheme: "file", language: "crepus" }],
    };
    const instance = new LanguageClient(
      "crepus-lsp",
      "Crepus Language Server",
      serverOptions,
      clientOptions
    );
    const current = { instance, ready: instance.start() };
    client = current;
    try { await current.ready; }
    catch (error) {
      if (client === current) client = undefined;
      throw error;
    }
  } catch (error) { reportStartError(error, token); }
}

export function activate(context: vscode.ExtensionContext) {
  active = true;
  void startClient(++generation);
  context.subscriptions.push(
    vscode.commands.registerCommand("crepus.restartLanguageServer", async () => {
      if (!active) return;
      const token = ++generation;
      try {
        do {
          await stopClient();
          // Another restart may have awaited a failed stop that restored its handle.
        } while (active && token === generation && client);
        if (active && token === generation) await startClient(token);
      } catch (error) { reportStartError(error, token); }
    }),
    new vscode.Disposable(() => {
      void deactivate().catch(() => {});
    })
  );
}

export async function deactivate(): Promise<void> {
  active = false;
  ++generation;
  do {
    await stopClient();
  } while (!active && client);
}
