import SwiftUI

public struct ViewIrRootView: View {
    private let ir: ViewIr

    public init(ir: ViewIr) {
        self.ir = ir
    }

    public var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(Array(ir.root.enumerated()), id: \.offset) { _, node in
                ViewNodeView(node: node)
            }
        }
        .padding()
    }
}

public struct ViewNodeView: View {
    let node: ViewNode

    public init(node: ViewNode) {
        self.node = node
    }

    public var body: some View {
        switch node {
        case .text(let content, let style):
            Text(content)
                .applyViewStyle(style, isText: true)

        case .stack(let axis, let spacing, let alignItems, let justifyContent, let style, let children):
            let gap = CGFloat(spacing ?? 8)
            let hAlign = horizontalAlignment(from: alignItems)
            Group {
                switch axis {
                case .column:
                    VStack(alignment: hAlign, spacing: gap) {
                        ForEach(Array(children.enumerated()), id: \.offset) { _, child in
                            ViewNodeView(node: child)
                        }
                    }
                case .row:
                    HStack(alignment: rowAlignment(from: alignItems), spacing: gap) {
                        ForEach(Array(children.enumerated()), id: \.offset) { _, child in
                            ViewNodeView(node: child)
                        }
                    }
                }
            }
            .applyViewStyle(style, isText: false)
            .modifier(JustifyOverlay(justify: justifyContent))

        case .button(let label, let onClick, let style):
            Button(action: {
                if let onClick { print("native shell: click \(onClick)") }
            }) {
                Text(label)
            }
            .applyViewStyle(style, isText: false)

        case .image(let src, let alt, let style):
            VStack(alignment: .leading) {
                Text(alt ?? src)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                Text("[image: \(src)]")
                    .font(.caption2)
            }
            .applyViewStyle(style, isText: false)

        case .scroll(let axis, let style, let children):
            let gap: CGFloat = 8
            ScrollView {
                switch axis {
                case .column:
                    VStack(alignment: .leading, spacing: gap) {
                        ForEach(Array(children.enumerated()), id: \.offset) { _, child in
                            ViewNodeView(node: child)
                        }
                    }
                case .row:
                    HStack(alignment: .top, spacing: gap) {
                        ForEach(Array(children.enumerated()), id: \.offset) { _, child in
                            ViewNodeView(node: child)
                        }
                    }
                }
            }
            .applyViewStyle(style, isText: false)

        case .slotRotate(let phrases, _, let style):
            // Full rotation needs a timer; show first phrase in the shell demo.
            Text(phrases.first ?? "")
                .applyViewStyle(style, isText: true)
        }
    }

    private func horizontalAlignment(from alignItems: String?) -> HorizontalAlignment {
        switch alignItems {
        case "end": return .trailing
        case "center": return .center
        default: return .leading
        }
    }

    private func rowAlignment(from alignItems: String?) -> VerticalAlignment {
        switch alignItems {
        case "end": return .bottom
        case "center": return .center
        case "stretch": return .center
        default: return .top
        }
    }
}

/// Best-effort `justify-content` for simple stacks (shell demo).
private struct JustifyOverlay: ViewModifier {
    let justify: String?

    func body(content: Content) -> some View {
        switch justify {
        case "center":
            content.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .center)
        case "end":
            content.frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .trailing)
        default:
            content
        }
    }
}

extension View {
    @ViewBuilder
    fileprivate func applyViewStyle(_ style: ViewStyle?, isText: Bool) -> some View {
        if let s = style {
            self.modifier(ViewStyleModifier(style: s, isText: isText))
        } else {
            self
        }
    }
}

private struct ViewStyleModifier: ViewModifier {
    let style: ViewStyle
    let isText: Bool

    func body(content: Content) -> some View {
        let pad = EdgeInsets(
            top: CGFloat(style.paddingTop ?? style.paddingVertical ?? style.padding ?? 0),
            leading: CGFloat(style.paddingLeft ?? style.paddingHorizontal ?? style.padding ?? 0),
            bottom: CGFloat(style.paddingBottom ?? style.paddingVertical ?? style.padding ?? 0),
            trailing: CGFloat(style.paddingRight ?? style.paddingHorizontal ?? style.padding ?? 0)
        )
        let hasPadding = style.padding != nil || style.paddingHorizontal != nil
            || style.paddingVertical != nil || style.paddingTop != nil || style.paddingBottom != nil
            || style.paddingLeft != nil || style.paddingRight != nil

        let base = content
            .modifier(ConditionalPadding(pad: pad, enabled: hasPadding))
            .background((style.backgroundColor.flatMap { Color(hex: $0) }) ?? Color.clear)
            .clipShape(RoundedRectangle(cornerRadius: CGFloat(style.cornerRadius ?? 0)))

        Group {
            if isText {
                textDecorations(base)
            } else {
                base
            }
        }
    }

    @ViewBuilder
    private func textDecorations(_ v: some View) -> some View {
        let w = style.fontWeight.map(fontWeightSwiftUI)
        let withFont = v
            .font(style.fontSize.map { .system(size: CGFloat($0)) } ?? .body)
            .fontWeight(w ?? .regular)
            .foregroundStyle((style.foregroundColor.flatMap { Color(hex: $0) }) ?? Color.primary)
            .italic(style.italic == true)
            .underline(style.underline == true)
            .strikethrough(style.strikethrough == true)
        switch style.textAlign {
        case "center":
            withFont.frame(maxWidth: .infinity, alignment: .center)
        case "trailing":
            withFont.frame(maxWidth: .infinity, alignment: .trailing)
        default:
            withFont
        }
    }
}

private struct ConditionalPadding: ViewModifier {
    let pad: EdgeInsets
    let enabled: Bool

    func body(content: Content) -> some View {
        if enabled {
            content.padding(pad)
        } else {
            content
        }
    }
}

private func fontWeightSwiftUI(_ w: UInt16) -> Font.Weight {
    switch w {
    case ..<300: return .thin
    case ..<400: return .light
    case ..<500: return .regular
    case ..<600: return .medium
    case ..<700: return .semibold
    case ..<800: return .bold
    default: return .heavy
    }
}

extension Color {
    init?(hex: String) {
        var t = hex.trimmingCharacters(in: .whitespaces)
        if t.hasPrefix("#") { t.removeFirst() }
        guard t.count == 6 || t.count == 8 else { return nil }
        var n: UInt64 = 0
        guard Scanner(string: t).scanHexInt64(&n) else { return nil }
        let a: Double
        let r: Double
        let g: Double
        let b: Double
        if t.count == 8 {
            a = Double((n & 0xFF00_0000) >> 24) / 255
            r = Double((n & 0x00FF_0000) >> 16) / 255
            g = Double((n & 0x0000_FF00) >> 8) / 255
            b = Double(n & 0x0000_00FF) / 255
        } else {
            a = 1
            r = Double((n & 0xFF0000) >> 16) / 255
            g = Double((n & 0x00FF00) >> 8) / 255
            b = Double(n & 0x0000FF) / 255
        }
        self = Color(.sRGB, red: r, green: g, blue: b, opacity: a)
    }
}

/// Loads bundled fixture and shows the tree (for Xcode Previews or embedding in an app).
public struct FixtureRootView: View {
    @State private var ir: ViewIr?
    @State private var errorText: String?

    private let bundle: Bundle

    public init(bundle: Bundle = NativeShellResources.bundle) {
        self.bundle = bundle
    }

    public var body: some View {
        Group {
            if let ir {
                ViewIrRootView(ir: ir)
            } else if let errorText {
                Text(errorText).foregroundStyle(.red)
            } else {
                ProgressView()
            }
        }
        .task {
            do {
                ir = try ViewIr.loadFixture(bundle: bundle)
            } catch {
                errorText = String(describing: error)
            }
        }
    }
}

#if DEBUG
#Preview("Fixture") {
    FixtureRootView()
}
#endif
