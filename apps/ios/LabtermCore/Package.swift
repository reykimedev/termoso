// swift-tools-version: 5.9
// Swift package wrapping the Rust `labterm-mobile` core for iOS.
//
// `Sources/LabtermCore/LabtermCore.swift` and `LabtermCoreFFI.xcframework`
// are produced by `apps/ios/build-core.sh` and are not checked in.
import PackageDescription

let package = Package(
    name: "LabtermCore",
    platforms: [.iOS(.v17)],
    products: [
        .library(name: "LabtermCore", targets: ["LabtermCore"]),
    ],
    targets: [
        .binaryTarget(
            name: "LabtermCoreFFI",
            path: "LabtermCoreFFI.xcframework"
        ),
        .target(
            name: "LabtermCore",
            dependencies: ["LabtermCoreFFI"],
            path: "Sources/LabtermCore",
            linkerSettings: [
                .linkedFramework("Security"),
                .linkedFramework("CoreFoundation"),
                .linkedFramework("SystemConfiguration"),
            ]
        ),
    ]
)
