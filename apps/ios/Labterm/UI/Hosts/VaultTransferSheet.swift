import SwiftUI
import LabtermCore

/// What to move or copy, and where to.
struct TransferRequestState: Identifiable, Hashable {
    var hostIds: [String] = []
    var groupIds: [String] = []
    var targetId: String
    var move: Bool
    var id: String { "\(move)-\(targetId)-\(hostIds.joined())-\(groupIds.joined())" }
}

/// "Move to vault" / "Copy to vault" for hosts and whole groups: choose whether
/// logins go along, check the preview, then run it (labterm_client::vaultmove).
struct VaultTransferSheet: View {
    @Bindable var model: VaultsModel
    let request: TransferRequestState
    @Environment(\.dismiss) private var dismiss
    @State private var shared: Bool?
    @State private var plan: TransferPlanCard?
    @State private var error: String?

    private var target: VaultInfo? { model.vaults.first { $0.id == request.targetId } }

    /// Logins go along by default, except from a personal vault into a team.
    private var withCredentials: Bool {
        shared ?? !(target?.kind == .team && model.selectedVault?.kind != .team)
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    Picker("Logins", selection: Binding(get: { withCredentials }, set: { shared = $0 })) {
                        Text("Logins go along (passwords, identities, keys)").tag(true)
                        Text("Leave logins out (everyone uses their own)").tag(false)
                    }
                    .pickerStyle(.inline)
                    .labelsHidden()
                }
                if let error {
                    Section { Text(error).foregroundStyle(.red) }
                } else if let plan {
                    if !plan.blocked.isEmpty {
                        Section("Refused") {
                            ForEach(Array(plan.blocked.enumerated()), id: \.offset) { _, b in
                                Text((b.label.isEmpty ? "" : "“\(b.label)”: ") + Self.blocker(b.reason))
                                    .foregroundStyle(.red)
                            }
                        }
                    } else {
                        Section {
                            Text("Groups: \(plan.groups) · hosts: \(plan.hosts)")
                            if request.move { Text("The originals are removed here.").foregroundStyle(.secondary) }
                        }
                        ForEach(Self.actions, id: \.self) { action in
                            let items = plan.dependencies.filter { $0.action == action }
                            if !items.isEmpty {
                                Section(Self.actionText(action)) {
                                    ForEach(items, id: \.id) { d in
                                        Text(Self.kindText(d.kind) + (d.label.isEmpty ? "" : " “\(d.label)”"))
                                    }
                                }
                            }
                        }
                    }
                } else {
                    ProgressView()
                }
            }
            .navigationTitle((request.move ? "Move to " : "Copy to ") + (target?.name ?? ""))
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
                ToolbarItem(placement: .confirmationAction) {
                    Button(request.move ? "Move" : "Copy") {
                        do {
                            try model.transfer(request, withCredentials: withCredentials)
                            dismiss()
                        } catch {
                            self.error = userMessage(for: error)
                        }
                    }
                    .disabled(plan == nil || !(plan?.blocked.isEmpty ?? false))
                }
            }
            .task(id: withCredentials) {
                plan = nil
                error = nil
                do { plan = try model.transferPlan(request, withCredentials: withCredentials) } catch { self.error = userMessage(for: error) }
            }
        }
    }

    private static let actions = ["copy", "reuse", "move_along", "leave_out", "unlink", "rewrite", "remove", "breaks"]

    private static func actionText(_ a: String) -> String {
        switch a {
        case "copy": "Copied along"
        case "reuse": "Already there, reused"
        case "move_along": "Moves along"
        case "leave_out": "Left out"
        case "unlink": "Unlinked: set it up in the target vault"
        case "rewrite": "Your personal overlay follows"
        case "remove": "Your personal overlay is removed"
        default: "Stays here and still points at moved hosts"
        }
    }

    private static func blocker(_ r: String) -> String {
        switch r {
        case "same_vault": "Already in this vault."
        case "mixed_vaults": "The selection comes from more than one vault."
        case "target_locked": "The target vault is locked."
        case "target_read_only": "You can only view the target vault."
        case "target_group": "The target group is in another vault."
        case "source_read_only": "Moving needs edit rights in this vault. Copy instead."
        case "connect_only": "Your access lets you connect, not move or copy it."
        case "hidden": "The selection holds hosts that are hidden from you."
        default: "Nothing to move."
        }
    }

    private static func kindText(_ k: String) -> String {
        switch k {
        case "identity": "Identity"
        case "ssh_key": "Key"
        case "snippet": "Snippet"
        case "proxy": "Proxy"
        case "host_chain": "Jump hosts"
        case "port_knocking": "Port knocking"
        case "transport": "VPN network"
        case "color_scheme": "Color scheme"
        case "tag": "Tag"
        case "pf_rule": "Port forwarding"
        case "host_overlay": "Personal overlay"
        default: k
        }
    }
}
