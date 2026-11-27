import SwiftUI

/// Type a command once and run it in this session, every session, a tab
/// group, a host group or a tag (host groups and tags include their
/// children). Each session gets the command once.
struct CommandSheet: View {
    @Bindable var store: SessionStore
    @Environment(\.dismiss) private var dismiss
    @State private var command = ""
    @State private var target: CommandTarget = .all

    var body: some View {
        NavigationStack {
            Form {
                Section("Send to") {
                    Picker("Send to", selection: $target) {
                        Text("This session").tag(CommandTarget.active)
                        Text("All sessions").tag(CommandTarget.all)
                        ForEach(store.tabGroups.values.sorted { $0.name < $1.name }) { group in
                            Text("Tab group: \(group.name)").tag(CommandTarget.group(group.id))
                        }
                        ForEach(CommandTarget.hostGroups(store.sessions.map(\.hostGroupPath)), id: \.self) { path in
                            Text("Group: \(path.joined(separator: " / "))").tag(CommandTarget.hostGroup(path))
                        }
                        ForEach(CommandTarget.tagGroups(store.sessions.flatMap(\.hostTags)), id: \.self) { tag in
                            Text("#\(tag)").tag(CommandTarget.tag(tag))
                        }
                    }
                    .pickerStyle(.navigationLink)
                    .accessibilityIdentifier("commandSheet.target")
                }
                Section("Command") {
                    TextField("Command", text: $command, axis: .vertical)
                        .font(.system(.body, design: .monospaced))
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .lineLimit(1 ... 6)
                        .accessibilityIdentifier("commandSheet.command")
                }
            }
            .navigationTitle("Send command")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    let count = store.sessions(for: target).count
                    Button(count == 1 ? "Send" : "Send to \(count)") {
                        store.send(command: command, to: target)
                        dismiss()
                    }
                    .disabled(count == 0 || command.trimmingCharacters(in: .whitespaces).isEmpty)
                    .accessibilityIdentifier("commandSheet.send")
                }
            }
        }
    }
}
