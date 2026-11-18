import { constants } from "node:fs";
import { access, stat } from "node:fs/promises";
import * as path from "node:path";

interface SearchEnvironment {
  platform?: NodeJS.Platform;
  env?: NodeJS.ProcessEnv;
}

/** Resolve a native server executable without invoking a shell. */
export async function resolveServerExecutable(
  workspace: string | undefined,
  configured: string,
  { platform = process.platform, env = process.env }: SearchEnvironment = {}
): Promise<string> {
  // Preserve explicit paths and command names exactly as the user supplied them.
  if (configured.trim()) return configured;
  const windows = platform === "win32";
  // Node cannot launch .bat/.cmd directly without a shell. Only native formats
  // from PATHEXT are eligible for automatic discovery.
  const extensions = windows
    ? (env.PATHEXT ?? ".COM;.EXE;.BAT;.CMD")
        .split(";")
        .filter((extension) => /^\.(exe|com)$/i.test(extension))
    : [""];
  const executable = async (candidate: string): Promise<boolean> => {
    try {
      if (!(await stat(candidate)).isFile()) return false;
      await access(candidate, windows ? constants.F_OK : constants.X_OK);
      return true;
    } catch {
      return false;
    }
  };
  const searchPath = env.PATH ?? (windows ? env.Path : undefined);
  for (let directory of searchPath?.split(windows ? ";" : ":") ?? []) {
    if (windows) directory = directory.replace(/^"(.*)"$/, "$1");
    for (const extension of extensions) {
      const candidate = path.resolve(directory, `crepus-lsp${extension}`);
      if (await executable(candidate)) return candidate;
    }
  }
  if (workspace) {
    const candidate = path.join(workspace, "target", "debug", windows ? "crepus-lsp.exe" : "crepus-lsp");
    if (await executable(candidate)) return candidate;
  }
  throw new Error(
    "Cannot find crepus-lsp on PATH or in the workspace target/debug directory. Install crepus-lsp or set crepus.languageServerPath."
  );
}
