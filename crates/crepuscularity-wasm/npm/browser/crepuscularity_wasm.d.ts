/* tslint:disable */
/* eslint-disable */

/**
 * Schema version of the IR this module emits; consumers should check it.
 */
export function ir_version(): number;

/**
 * Parse `.crepus` source into View IR, serialized as JSON.
 *
 * `context_json`, when present, must be a JSON object whose values are bound
 * as template variables.
 */
export function parse_crepus_json(source: string, context_json?: string | null): string;

/**
 * Parse a template into View IR, choosing the frontend from `filename`.
 *
 * The parser dispatches on the file extension, so `.crepus`, `.jsx`/`.tsx`,
 * `.svelte`, `.vue`, `.astro` and Angular component templates
 * (`*.component.html`, `*.ng.html`, `*.ng`) all reach the same IR through
 * their own frontend.
 */
export function parse_template_json(source: string, filename?: string | null, context_json?: string | null): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly ir_version: () => number;
    readonly parse_crepus_json: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly parse_template_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
