// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "CrepuscularityPlugins",
    products: [
        .library(name: "CrepuscularityPlugin", targets: ["CrepuscularityPlugin"])
    ],
    targets: [
        .target(
            name: "CrepuscularityPlugin",
            path: "swift",
            sources: ["CrepuscularityPlugin.swift"]
        ),
        .testTarget(
            name: "CrepuscularityPluginTests",
            dependencies: ["CrepuscularityPlugin"],
            path: "swift-tests"
        )
    ]
)
