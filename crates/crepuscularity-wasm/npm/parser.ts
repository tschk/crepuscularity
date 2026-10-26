import type { ViewIr } from "./types.js";

export type {
  LabelPart,
  PickerOption,
  StackAxis,
  TabItem,
  ViewIr,
  ViewNode,
  ViewNodeKind,
  ViewStyle,
} from "./types.js";

export type CrepusContext = Record<string, string | number | boolean | null>;
type Bindings = Pick<typeof import("./wasm/crepuscularity_wasm.js"),
  "ir_version" | "parse_crepus_json" | "parse_template_json">;

export function createParser({ ir_version, parse_crepus_json, parse_template_json }: Bindings) {
  /// Schema version of the View IR this build of the parser emits.
  const IR_VERSION: number = ir_version();

  function validateUnicode(value: string, label: string): void {
    if (/[\uD800-\uDFFF]/u.test(value)) {
      throw new TypeError(`${label} contains an unpaired UTF-16 surrogate`);
    }
  }

  function serializeContext(context?: CrepusContext): string | undefined {
    if (!context) return undefined;
    return JSON.stringify(context, (_key, value: unknown) => {
      if (typeof value === "number" && !Number.isFinite(value)) {
        throw new TypeError("context numbers must be finite");
      }
      return value;
    });
  }

  /**
   * Parse `.crepus` source into View IR using the same Rust parser that backs the
   * `crepus` CLI.
   */
  function parseCrepus(source: string, context?: CrepusContext): ViewIr {
    validateUnicode(source, "source");
    const json = parse_crepus_json(
      source,
      serializeContext(context),
    );
    return JSON.parse(json) as ViewIr;
  }

  /**
   * Parse any supported template syntax into View IR. The frontend is chosen from
   * `filename`'s extension or suffix, so `.crepus`, `.jsx`, `.tsx`, `.svelte`,
   * `.vue`, `.astro`, and Angular component templates each use their own parser.
   */
  function parseTemplate(
    source: string,
    filename?: string,
    context?: CrepusContext,
  ): ViewIr {
    validateUnicode(source, "source");
    if (filename !== undefined) validateUnicode(filename, "filename");
    const json = parse_template_json(
      source,
      filename,
      serializeContext(context),
    );
    return JSON.parse(json) as ViewIr;
  }
  return { IR_VERSION, parseCrepus, parseTemplate };
}
