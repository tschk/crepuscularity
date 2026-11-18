// Compiled by runtime-as-strings.test.js with a supplied AssemblyScript compiler.
export function runtime_version(): string { return "AS\u0000é😀\ud800"; }
export function browser_program(): string { return "\ufeffprogram\u0000😀"; }
export function app_manifest(): string { return '{"name":"é😀"}'; }
export function extract_specs(text: string): string { __collect(); return text; }
export function render_frontend(request: string): string { __collect(); return request; }
export function render_popup(state: string): string { __collect(); return state; }
export function handle_popup_action(action: string, data: string): string {
  __collect();
  return '{"actionLength":' + action.length.toString() + ',"data":' + data + '}';
}
