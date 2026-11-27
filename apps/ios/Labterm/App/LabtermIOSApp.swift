import SwiftUI
import LabtermCore

@main
struct LabtermIOSApp: App {
    @State private var container: AppContainer

    init() {
        let launch = LaunchOptions.current
        // One log file per launch (last ten kept, secrets removed) for Settings → Debug log.
        // Caches: not backed up, and the system may clear it when space runs low.
        let logDir = FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("debug-logs", isDirectory: true)
        initLoggingWithFiles(verbose: launch.verboseLogging, logDir: logDir.path)
        _container = State(initialValue: AppContainer(launch: launch))
    }

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(container)
        }
    }
}

/// Process arguments understood by the app. UI tests pass `--ui-test` to get
/// a throw-away profile (temporary directory, ephemeral master key, welcome
/// shown) so they never touch the real vault or Keychain.
struct LaunchOptions: Sendable {
    var ephemeralProfile = false
    var verboseLogging = false

    static let current = LaunchOptions(arguments: CommandLine.arguments)

    init(arguments: [String]) {
        ephemeralProfile = arguments.contains("--ui-test")
        verboseLogging = arguments.contains("--verbose") || ephemeralProfile
    }
}
