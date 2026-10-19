# npm publishing helper — 19 October 2026

Run from this folder:

```sh
bun publish.ts --login --publish
```

`--login` opens npm's browser login. No token is embedded, printed or requested in chat. Bun handles package checks, packing and publishing. Existing registry versions are skipped and their tarball availability is checked. A failing package blocks consumers requiring its unavailable version; independent validated packages can still publish. The command exits nonzero if any candidate remains blocked.

To inspect the live plan without logging in, building or publishing:

```sh
bun publish.ts
```

To publish only the prepared parser:

```sh
bun publish.ts --login --publish --only crepuscularity-wasm
```

`--prepare` runs the same local checks/packing and publish dry runs without uploading. `--only` filters package names; required dependencies must still resolve from npm or another prepared package. `--projects /path/to/projects` changes the local project directory.

## What is included

- `@tschk/crepuscularity-wasm@0.1.4`: a frozen SHA-512-checked tarball from reviewed release source `053bde61`. Its actual packed entry passed all six frontend calls under Bun and Node 22 with TypeScript stripping; packaging dry run passed. The nested CommonJS marker is included. This publishes that artifact without rebuilding or running prepublish scripts.
- The 26 Moonshine packages: local `0.4.43`, npm `0.4.4` at inspection. Before packing, the script requires a clean source tree and passes `test`, `typecheck`, `typecheck:native`, `check:packages`, `check:catalog` and `check:adapters`. **The current Moonshine checkout is dirty and these packages are blocked.** It must be reviewed and committed before publication. The script never reverts files, commits, bumps versions, or deletes credentials.
- `@tschk/flowtoken-svelte`: npm/local `0.0.2`, but the actual npm artifact lacks current tokenization and animation changes. It needs a reviewed version bump, such as a patch, before those changes can be published. The script reports this and will recognize a newer local version automatically; check-types, tests and build run before packing.
- `@tschk/svelte-streamdown`: npm/local `0.0.1`. The inspected runtime behavior is present in the published artifact; local differences include an explicit type annotation, README and build-script cleanup. An optional patch needs a new version. Its checks/build recipe is included for a future bump.
- `@trychert/facetime` and `@trychert/facetime-opensource`: public maintainership verified, but no matching local package source was found. Listed for visibility; no guessed release is attempted.

This is a live check of 31 publicly listed packages maintained by npm user `undivisible`, not an authenticated inventory of private packages. Versions and maintainers are rechecked on every run. Matching version numbers mean that version is already immutable on npm, not that every local source change has shipped. Required packed dependency ranges are checked even when `--only` excludes their packages. Prereleases use the `next` tag; stable versions use `latest`.

No package has been uploaded by preparation or tests. npm currently returns 401 until authentication is refreshed.

## Files and verification

`release.json` records source locations and the parser tarball checksum. `inventory.txt` is a dated observation; rerun the script for current data. `parser-prepare.txt`, `packed-acceptance.txt` and `svelte-artifact-comparison.json` record scoped checks. `bun test publish.test.ts` exercises version decisions, dependency ordering/cycles, unresolved local dependencies, filtered consumers and required peers.

The parser tarball can be reproduced from the reviewed release worktree using `bun pm pack --ignore-scripts --filename /absolute/path/tschk-crepuscularity-wasm-0.1.4.tgz` in its `crates/crepuscularity-wasm/npm` directory. Do not replace the frozen artifact or its checksum with an untested rebuild. Registry uploads cannot be overwritten; successful earlier uploads remain published if a later upload fails, and rerunning skips existing versions.

Documentation: [Bun packing/publishing](https://bun.com/docs/pm/cli/publish), [npm browser login](https://docs.npmjs.com/cli/v11/commands/npm-login/).
