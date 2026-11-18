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
