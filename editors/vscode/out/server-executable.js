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
exports.resolveServerExecutable = resolveServerExecutable;
const node_fs_1 = require("node:fs");
const promises_1 = require("node:fs/promises");
const path = __importStar(require("node:path"));
/** Resolve a native server executable without invoking a shell. */
async function resolveServerExecutable(workspace, configured, { platform = process.platform, env = process.env } = {}) {
    // Preserve explicit paths and command names exactly as the user supplied them.
    if (configured.trim())
        return configured;
    const windows = platform === "win32";
    // Node cannot launch .bat/.cmd directly without a shell. Only native formats
    // from PATHEXT are eligible for automatic discovery.
    const extensions = windows
        ? (env.PATHEXT ?? ".COM;.EXE;.BAT;.CMD")
            .split(";")
            .filter((extension) => /^\.(exe|com)$/i.test(extension))
        : [""];
    const executable = async (candidate) => {
        try {
            if (!(await (0, promises_1.stat)(candidate)).isFile())
                return false;
            await (0, promises_1.access)(candidate, windows ? node_fs_1.constants.F_OK : node_fs_1.constants.X_OK);
            return true;
        }
        catch {
            return false;
        }
    };
    const searchPath = env.PATH ?? (windows ? env.Path : undefined);
    for (let directory of searchPath?.split(windows ? ";" : ":") ?? []) {
        if (windows)
            directory = directory.replace(/^"(.*)"$/, "$1");
        for (const extension of extensions) {
            const candidate = path.resolve(directory, `crepus-lsp${extension}`);
            if (await executable(candidate))
                return candidate;
        }
    }
    if (workspace) {
        const candidate = path.join(workspace, "target", "debug", windows ? "crepus-lsp.exe" : "crepus-lsp");
        if (await executable(candidate))
            return candidate;
    }
    throw new Error("Cannot find crepus-lsp on PATH or in the workspace target/debug directory. Install crepus-lsp or set crepus.languageServerPath.");
}
