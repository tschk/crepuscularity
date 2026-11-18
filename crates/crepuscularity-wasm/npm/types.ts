export type StackAxis = "row" | "column";

export interface PickerOption {
  value: string;
  label: string;
}

export interface TabItem {
  value: string;
  label: string;
  icon?: string;
  children: ViewNode[];
}

export type LabelPart =
  | { kind: "literal"; value: string }
  | { kind: "expr"; value: string };

export interface ViewStyle {
  htmlTag?: string;
  htmlDisplayContents?: boolean;
  htmlFor?: string;
  ariaLabel?: string;
  formName?: string;
  formValue?: string;
  inputType?: string;
  required?: boolean;
  formMin?: string;
  formMax?: string;
  formStep?: string;
  id?: string;
  classes?: string[];

  padding?: number;
  paddingHorizontal?: number;
  paddingVertical?: number;
  paddingTop?: number;
  paddingBottom?: number;
  paddingLeft?: number;
  paddingRight?: number;

  margin?: number;
  marginHorizontal?: number;
  marginVertical?: number;
  marginTop?: number;
  marginBottom?: number;
  marginLeft?: number;
  marginRight?: number;

  width?: number;
  height?: number;
  minWidth?: number;
  minHeight?: number;
  maxWidth?: number;
  maxHeight?: number;
  /** @deprecated Not emitted or interpreted by the current View IR schema. */
  widthFraction?: number;
  /** @deprecated Not emitted or interpreted by the current View IR schema. */
  heightFraction?: number;
  aspectRatio?: number;

  fontSize?: number;
  fontWeight?: number;
  textAlign?: string;
  lineHeight?: number;
  letterSpacing?: number;
  textTransform?: string;
  fontFamily?: string;
  italic?: boolean;
  underline?: boolean;
  strikethrough?: boolean;

  foregroundColor?: string;
  backgroundColor?: string;

  cornerRadius?: number;
  borderWidth?: number;
  borderColor?: string;

  opacity?: number;
  hidden?: boolean;
  overflowHidden?: boolean;

  flexGrow?: number;
  flexShrink?: number;
  alignSelf?: string;
  flexWrap?: boolean;
  flexDirection?: string;

  position?: string;
  zIndex?: number;
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;

  translateX?: number;
  translateY?: number;
  scaleX?: number;
  scaleY?: number;
  rotate?: number;

  shadowColor?: string;
  shadowRadius?: number;
  shadowOffsetX?: number;
  shadowOffsetY?: number;

  textOverflow?: string;
  whiteSpace?: string;
  lineClamp?: number;
  cursor?: string;
  userSelect?: string;

  backgroundGradientDirection?: string;
  backgroundGradientFrom?: string;
  backgroundGradientTo?: string;

  objectFit?: string;
  objectPosition?: string;
}

interface Styled {
  style?: ViewStyle;
}

export type ViewNode =
  | ({ kind: "text"; content: string; bind?: string } & Styled)
  | ({
      kind: "if";
      condition: string;
      thenChildren: ViewNode[];
      elseChildren?: ViewNode[];
    } & Styled)
  | ({
      kind: "forEach";
      bind: string;
      itemName: string;
      itemBody: ViewNode[];
    } & Styled)
  | ({
      kind: "stack";
      axis: StackAxis;
      spacing?: number;
      alignItems?: string;
      justifyContent?: string;
      onLongPress?: string;
      children: ViewNode[];
    } & Styled)
  | ({
      kind: "button";
      label: string;
      labelParts?: LabelPart[];
      onClick?: string;
      onLongPress?: string;
    } & Styled)
  | ({
      kind: "toggle";
      label: string;
      bind?: string;
      checked: boolean;
      onChange?: string;
      onLongPress?: string;
    } & Styled)
  | ({
      kind: "checkbox";
      label: string;
      bind?: string;
      checked: boolean;
      onChange?: string;
    } & Styled)
  | ({
      kind: "slider";
      label?: string;
      bind?: string;
      value: number;
      min: number;
      max: number;
      step?: number;
      onChange?: string;
    } & Styled)
  | ({ kind: "progress"; label?: string; value: number; max: number } & Styled)
  | ({
      kind: "meter";
      label?: string;
      value: number;
      min: number;
      max: number;
    } & Styled)
  | ({ kind: "badge"; label: string; tone?: string } & Styled)
  | ({ kind: "divider"; axis: StackAxis } & Styled)
  | ({ kind: "spacer"; size?: number } & Styled)
  | ({
      kind: "dropzone";
      label: string;
      accept?: string;
      onDrop?: string;
      children: ViewNode[];
    } & Styled)
  | ({
      kind: "filePicker";
      label: string;
      accept?: string[];
      multiple?: boolean;
      onPick?: string;
    } & Styled)
  | ({
      kind: "image";
      src: string;
      srcBind?: string;
      alt?: string;
      placeholder?: string;
      onLongPress?: string;
    } & Styled)
  | ({
      kind: "link";
      href: string;
      hrefBind?: string;
      target?: string;
      rel?: string;
      children: ViewNode[];
    } & Styled)
  | ({ kind: "webView"; src: string } & Styled)
  | ({ kind: "scroll"; axis: StackAxis; onLongPress?: string; children: ViewNode[] } & Styled)
  | ({ kind: "list"; ordered: boolean; children: ViewNode[] } & Styled)
  | ({ kind: "listItem"; onLongPress?: string; children: ViewNode[] } & Styled)
  | ({ kind: "slotRotate"; phrases: string[]; intervalMs: number } & Styled)
  | ({
      kind: "input";
      placeholder: string;
      bind: string;
      multiline?: boolean;
      secure?: boolean;
      onInput?: string;
      onChange?: string;
    } & Styled)
  | ({
      kind: "picker";
      bind: string;
      options: PickerOption[];
      onChange?: string;
    } & Styled)
  | ({
      kind: "tabs";
      bind: string;
      tabs: TabItem[];
      onChange?: string;
    } & Styled);

export type ViewNodeKind = ViewNode["kind"];

export interface ViewIr {
  version: number;
  root: ViewNode[];
}
