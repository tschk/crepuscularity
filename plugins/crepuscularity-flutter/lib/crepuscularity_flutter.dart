/// Flutter runtime renderer for Crepuscularity — the Flutter counterpart to the
/// SwiftUI / Jetpack Compose native shells. Two entry points, both producing
/// Flutter widgets:
///
/// * [CrepusView.fromIr] — render decoded **View IR** JSON.
/// * [CrepusView.fromSource] — render `.crepus` source via a pure-Dart parser.
///
/// The package is generic and action-agnostic: interactive actions are surfaced
/// to the host through [CrepusActionCallback]; the package never interprets or
/// evaluates them.
library;

export 'package:crepuscularity_components/crepuscularity_components.dart'
    show
        DitherSparkline,
        DitherColor,
        DitherVariant,
        ditherColorFromName,
        ditherVariantFromName,
        parseSparklineValues;
export 'src/crepus_parser.dart' show viewIrFromSource, IR_VERSION;
export 'src/renderer.dart' show CrepusView, CrepusTheme, CrepusActionCallback;
export 'src/view_ir.dart'
    show
        ViewIr,
        ViewNode,
        ViewStyle,
        StackAxis,
        CrepusLimits,
        kAllowedKinds,
        childrenOf,
        crepusExceedsLimits,
        TextNode,
        StackNode,
        ScrollNode,
        ButtonNode,
        ToggleNode,
        CheckboxNode,
        ProgressNode,
        TimerNode,
        parseCrepusDuration,
        MeterNode,
        SparklineNode,
        BadgeNode,
        DividerNode,
        SpacerNode,
        ImageNode,
        IfNode,
        ForEachNode,
        ListNode,
        ListItemNode,
        LinkNode,
        UnsupportedNode;
