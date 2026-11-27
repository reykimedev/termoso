import SwiftUI
import LabtermCore

/// The core's debug log files (one per launch, last ten kept, secrets
/// removed): pick a launch, read it, share it for a bug report.
struct DebugLogView: View {
    @State private var files: [DebugLogFile] = []
    @State private var selected: String?
    @State private var lines: [String] = []
    @State private var error: String?

    var body: some View {
        List {
            Section {
                Picker("Log file", selection: $selected) {
                    ForEach(files, id: \.name) { file in
                        Text(label(file)).tag(Optional(file.name))
                    }
                }
                .accessibilityIdentifier("debugLog.file")
            }
            Section {
                if let error {
                    Text(error).foregroundStyle(.red)
                } else if lines.isEmpty {
                    Text("Nothing logged here yet.").foregroundStyle(.secondary)
                } else {
                    ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                        Text(line)
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(color(line))
                            .textSelection(.enabled)
                    }
                }
            }
        }
        .navigationTitle("Debug log")
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Button {
                    reload()
                } label: {
                    Label("Refresh", systemImage: "arrow.clockwise")
                }
                ShareLink(item: lines.joined(separator: "\n")) {
                    Label("Share", systemImage: "square.and.arrow.up")
                }
                .disabled(lines.isEmpty)
            }
        }
        .onAppear { reload() }
        .onChange(of: selected) { read() }
    }

    private func reload() {
        files = (try? debugLogFiles()) ?? []
        if selected == nil || !files.contains(where: { $0.name == selected }) {
            selected = files.first(where: { $0.current })?.name ?? files.first?.name
        }
        read()
    }

    private func read() {
        guard let name = selected else {
            lines = []
            return
        }
        do {
            lines = try debugLogRead(name: name)
            error = nil
        } catch {
            lines = []
            self.error = userMessage(for: error)
        }
    }

    private func label(_ file: DebugLogFile) -> String {
        let size = ByteCountFormatter.string(fromByteCount: Int64(file.size), countStyle: .file)
        if file.current { return "This launch · \(size)" }
        let date = Date(timeIntervalSince1970: TimeInterval(file.modified))
        return "\(date.formatted(date: .abbreviated, time: .shortened)) · \(size)"
    }

    private func color(_ line: String) -> Color {
        let head = line.prefix(48)
        if head.contains("ERROR") { return .red }
        if head.contains("WARN") { return .orange }
        return .primary
    }
}
