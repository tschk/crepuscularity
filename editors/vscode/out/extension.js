"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const server_executable_1 = require("./server-executable");
const vscode = __importStar(require("vscode"));
const node_1 = require("vscode-languageclient/node");
let client;
let generation = 0;
let active = false;
let stopping = Promise.resolve();
function stopClient() {
    const current = client;
    client = undefined;
    if (current) {
        const pending = stopping.then(async () => {
            // LanguageClient rejects stop() while start() is still in progress.
            try {
                await current.ready;
            }
            catch {
                return;
            }
            try {
                await current.instance.stop();
            }
            catch (error) {
                // Retain the handle so a later restart can retry shutdown first.
                if (!client)
                    client = current;
                throw error;
            }
        });
        // Report this attempt's failure, without poisoning future shutdown attempts.
        stopping = pending.catch(() => { });
        return pending;
    }
    return stopping;
}
function reportStartError(error, token) {
    if (active && token === generation) {
        void vscode.window.showErrorMessage(`Crepus language server failed to start: ${String(error)}`);
    }
}
async function startClient(token) {
    try {
        const folder = vscode.workspace.workspaceFolders?.[0];
        const cfg = vscode.workspace.getConfiguration("crepus");
        const command = await (0, server_executable_1.resolveServerExecutable)(folder?.uri.fsPath, cfg.get("languageServerPath", ""));
        if (!active || token !== generation)
            return;
        const serverOptions = {
            run: { command, args: ["--stdio"], transport: node_1.TransportKind.stdio },
            debug: { command, args: ["--stdio"], transport: node_1.TransportKind.stdio },
        };
        const clientOptions = {
            documentSelector: [{ scheme: "file", language: "crepus" }],
        };
        const instance = new node_1.LanguageClient("crepus-lsp", "Crepus Language Server", serverOptions, clientOptions);
        const current = { instance, ready: instance.start() };
        client = current;
        try {
            await current.ready;
        }
        catch (error) {
            if (client === current)
                client = undefined;
            throw error;
        }
    }
    catch (error) {
        reportStartError(error, token);
    }
}
function activate(context) {
    active = true;
    void startClient(++generation);
    context.subscriptions.push(vscode.commands.registerCommand("crepus.restartLanguageServer", async () => {
        if (!active)
            return;
        const token = ++generation;
        try {
            do {
                await stopClient();
                // Another restart may have awaited a failed stop that restored its handle.
            } while (active && token === generation && client);
            if (active && token === generation)
                await startClient(token);
        }
        catch (error) {
            reportStartError(error, token);
        }
    }), new vscode.Disposable(() => {
        void deactivate().catch(() => { });
    }));
}
async function deactivate() {
    active = false;
    ++generation;
    do {
        await stopClient();
    } while (!active && client);
}
