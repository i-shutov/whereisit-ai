// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "AtlasCatalog",
    platforms: [.macOS(.v13)],
    products: [.executable(name: "AtlasCatalog", targets: ["AtlasCatalog"])],
    targets: [
        .systemLibrary(name: "CAtlas", path: "Sources/CAtlas"),
        .executableTarget(name: "AtlasCatalog", dependencies: ["CAtlas"], linkerSettings: [.linkedLibrary("atlas_ffi")])
    ]
)
