import SwiftUI

public enum CrepusActions {
    public static var dispatch: (String) -> String = { _ in "{}" }
    public static var resultSink: (String) -> Void = { _ in }

    public static func perform(_ action: String) {
        resultSink(dispatch(action))
    }
}

public struct CrepusGeneratedView: View {
    public init() {}

    public var body: some View {
        VStack(alignment: .leading, spacing: 16.0) {
            VStack(alignment: .leading, spacing: 8.0) {
                Text("Crepus Mobile")
                    .font(.system(size: 12.0))
                    .foregroundStyle(Color(red: 0.780, green: 0.824, blue: 0.996, opacity: 1.000))
                Text("Launch Control")
                    .font(.system(size: 24.0))
                    .fontWeight(.bold)
                    .foregroundStyle(Color(red: 1.000, green: 1.000, blue: 1.000, opacity: 1.000))
                Text("A full-screen SwiftUI and Compose app rendered from one .crepus file.")
                    .font(.system(size: 14.0))
                    .foregroundStyle(Color(red: 0.898, green: 0.906, blue: 0.922, opacity: 1.000))
            }
            HStack(alignment: .top, spacing: 16.0) {
                VStack(alignment: .leading, spacing: 8.0) {
                    Text("Build")
                        .font(.system(size: 12.0))
                        .foregroundStyle(Color(red: 0.859, green: 0.918, blue: 0.996, opacity: 1.000))
                    Text("Ready")
                        .font(.system(size: 24.0))
                        .fontWeight(.bold)
                        .foregroundStyle(Color(red: 1.000, green: 1.000, blue: 1.000, opacity: 1.000))
                }
                    .padding(16)
                    .background(Color(red: 0.145, green: 0.388, blue: 0.922, opacity: 1.000))
                    .clipShape(RoundedRectangle(cornerRadius: 8.0))
                VStack(alignment: .leading, spacing: 8.0) {
                    Text("Runtime")
                        .font(.system(size: 12.0))
                        .foregroundStyle(Color(red: 0.820, green: 0.980, blue: 0.898, opacity: 1.000))
                    Text("Live")
                        .font(.system(size: 24.0))
                        .fontWeight(.bold)
                        .foregroundStyle(Color(red: 1.000, green: 1.000, blue: 1.000, opacity: 1.000))
                }
                    .padding(16)
                    .background(Color(red: 0.020, green: 0.588, blue: 0.412, opacity: 1.000))
                    .clipShape(RoundedRectangle(cornerRadius: 8.0))
            }
            VStack(alignment: .leading, spacing: 8.0) {
                Text("Today")
                    .font(.system(size: 14.0))
                    .fontWeight(.semibold)
                    .foregroundStyle(Color(red: 1.000, green: 1.000, blue: 1.000, opacity: 1.000))
                Text("Edit views/main.crepus and run crepus mobile dev for hot reload.")
                    .font(.system(size: 14.0))
                    .foregroundStyle(Color(red: 0.898, green: 0.906, blue: 0.922, opacity: 1.000))
                Text("Ship one View IR tree to iOS and Android.")
                    .font(.system(size: 14.0))
                    .foregroundStyle(Color(red: 0.898, green: 0.906, blue: 0.922, opacity: 1.000))
            }
                .padding(16)
                .background(Color(red: 0.122, green: 0.161, blue: 0.216, opacity: 1.000))
                .clipShape(RoundedRectangle(cornerRadius: 8.0))
            HStack(alignment: .top, spacing: 16.0) {
                Button(action: { CrepusActions.perform("sync") }) {
                    Text("Sync")
                }
                    .padding(16)
                    .background(Color(red: 1.000, green: 1.000, blue: 1.000, opacity: 1.000))
                    .clipShape(RoundedRectangle(cornerRadius: 8.0))
                Button(action: { CrepusActions.perform("preview") }) {
                    Text("Preview")
                }
                    .padding(16)
                    .background(Color(red: 0.200, green: 0.255, blue: 0.333, opacity: 1.000))
                    .clipShape(RoundedRectangle(cornerRadius: 8.0))
            }
        }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .padding(24)
            .background(Color(red: 0.063, green: 0.086, blue: 0.141, opacity: 1.000))
    }
}
