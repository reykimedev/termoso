import SwiftUI
import LabtermCore

/// Access overview (policy viewer): for every host of the vault, the login,
/// VPN network, jump hosts and proxy a connection from this device ends up
/// with, where each comes from, the setups worth a second look and, in team
/// vaults, who can see the host. Worked out on the device from the
/// decrypted vault; who can see a host joins the team's access rules.
struct AccessOverviewView: View {
    @Bindable var model: VaultsModel
    let vault: VaultInfo

    @State private var policy: VaultPolicyCard?
    @State private var access: VaultAccessCard?
    @State private var failure: String?
    @State private var search = ""
    @State private var onlyRisks = false
    @State private var editor: HostEditorTarget?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if let failure {
                    Label(failure, systemImage: "exclamationmark.triangle")
                        .foregroundStyle(.red)
                        .padding(14)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .card()
                } else if let policy {
                    header(policy)
                    let shown = filtered(policy)
                    CardSection(onlyRisks ? "Needs a look" : "Hosts") {
                        if shown.isEmpty {
                            Text(policy.hosts.isEmpty ? "No hosts in this vault yet." : "No host matches.")
                                .foregroundStyle(.secondary)
                                .padding(14)
                                .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        ForEach(Array(shown.enumerated()), id: \.element.hostId) { index, host in
                            if index > 0 { RowDivider() }
                            NavigationLink {
                                AccessHostDetailView(
                                    host: host,
                                    team: policy.team,
                                    access: access,
                                    labelOf: { labelOf(policy, $0) },
                                    onEdit: { editor = .edit(id: host.hostId) }
                                )
                            } label: {
                                AccessHostRow(host: host)
                            }
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("access.host.\(host.label)")
                            .contextMenu { hostMenu(host) }
                        }
                    }
                } else {
                    ProgressView().frame(maxWidth: .infinity).padding(32)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        }
        .pageBackground()
        .navigationTitle("Access overview")
        .refreshable { load() }
        .task { load() }
        .sheet(item: $editor, onDismiss: load) { target in
            HostEditorView(model: model, target: target)
        }
    }

    private func header(_ p: VaultPolicyCard) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            Text("\(vault.name) · hosts: \(p.hosts.count) · need a look: \(p.hosts.filter { !$0.risks.isEmpty }.count)")
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Text(p.team
                ? "Worked out on this device from the decrypted vault. Access rules live on the server, which blocks changes below Edit; every member's device can still decrypt all items of this vault."
                : "Worked out on this device. Only you can see the items of this vault.")
                .font(.footnote)
                .foregroundStyle(.secondary)
            Picker("Show", selection: $onlyRisks) {
                Text("All hosts").tag(false)
                Text("Needs a look").tag(true)
            }
            .pickerStyle(.segmented)
            .accessibilityIdentifier("access.filter")
            SearchField(prompt: "Search", text: $search, identifier: "access.search")
        }
    }

    @ViewBuilder
    private func hostMenu(_ host: HostPolicyCard) -> some View {
        Button("Edit host", systemImage: "pencil") { editor = .edit(id: host.hostId) }
        ForEach(Array(host.risks.enumerated()), id: \.offset) { _, risk in
            Button("Fix: \(riskText(risk))", systemImage: "wrench") { editor = .edit(id: host.hostId) }
        }
    }

    private func filtered(_ p: VaultPolicyCard) -> [HostPolicyCard] {
        let q = search.trimmingCharacters(in: .whitespaces).lowercased()
        return p.hosts.filter { h in
            (!onlyRisks || !h.risks.isEmpty) &&
                (q.isEmpty || h.label.lowercased().contains(q) || h.address.lowercased().contains(q)
                    || h.groupPath.contains { $0.lowercased().contains(q) })
        }
    }

    private func labelOf(_ p: VaultPolicyCard, _ id: String) -> String {
        p.groups.first { $0.id == id }?.label ?? p.hosts.first { $0.hostId == id }?.label ?? String(id.prefix(8))
    }

    private func load() {
        do {
            policy = try model.repository.app.vaultPolicy(vaultId: vault.id)
            failure = nil
        } catch {
            failure = userMessage(for: error)
        }
        guard let teamId = vault.teamId else { return }
        let app = model.repository.app
        let vaultId = vault.id
        Task {
            // Reads members and rules from the server: off the main actor.
            let result = await Task.detached { try? app.vaultHostAccess(teamId: teamId, vaultId: vaultId) }.value
            if let result { access = result }
        }
    }
}

private struct AccessHostRow: View {
    let host: HostPolicyCard

    var body: some View {
        let danger = host.risks.contains { $0.severity == "danger" }
        HStack(alignment: .top, spacing: 12) {
            SymbolTile(
                systemImage: host.risks.isEmpty ? "server.rack" : "exclamationmark.triangle",
                tint: host.risks.isEmpty ? .accentColor : (danger ? .red : .orange)
            )
            VStack(alignment: .leading, spacing: 2) {
                Text(host.label).lineLimit(1)
                Text((host.groupPath + [host.address]).joined(separator: " / "))
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                Text([loginText(host), networkText(host), jumpText(host)].joined(separator: " · "))
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .lineLimit(2)
                if !host.risks.isEmpty {
                    Text("Needs a look: \(host.risks.count)")
                        .font(.caption.weight(.medium))
                        .foregroundStyle(danger ? .red : .orange)
                }
            }
            Spacer(minLength: 8)
            Image(systemName: "chevron.right")
                .font(.footnote.weight(.semibold))
                .foregroundStyle(.tertiary)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .contentShape(Rectangle())
    }
}

private struct AccessHostDetailView: View {
    let host: HostPolicyCard
    let team: Bool
    let access: VaultAccessCard?
    let labelOf: (String) -> String
    let onEdit: () -> Void

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                if !host.risks.isEmpty {
                    CardSection("Needs a look") {
                        ForEach(Array(host.risks.enumerated()), id: \.offset) { index, risk in
                            if index > 0 { RowDivider(inset: 14) }
                            fact(riskText(risk), sourceText(risk.fix), color: risk.severity == "danger" ? .red : .primary)
                        }
                    }
                }
                CardSection("How it connects") {
                    fact(loginText(host), "Login · " + sourceText(host.auth.source))
                    RowDivider(inset: 14)
                    fact(networkText(host), "VPN network · " + sourceText(host.network?.source))
                    RowDivider(inset: 14)
                    fact(jumpText(host), "Jump hosts · " + sourceText(host.jumpSource))
                    if let proxy = host.proxy {
                        RowDivider(inset: 14)
                        fact(proxy.label, "Proxy · " + sourceText(proxy.source))
                    }
                }
                if team { whoCanSee }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        }
        .pageBackground()
        .navigationTitle(host.label)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button("Edit host", systemImage: "pencil", action: onEdit)
            }
        }
    }

    @ViewBuilder
    private var whoCanSee: some View {
        let entry = access?.hosts.first { $0.hostId == host.hostId }
        CardSection("Who can see this host") {
            if let entry {
                ForEach(Array(entry.members.enumerated()), id: \.element.userId) { index, m in
                    if index > 0 { RowDivider(inset: 14) }
                    let depends = m.dependsOnGroups.isEmpty ? "" : " · may differ for members of " + m.dependsOnGroups.joined(separator: ", ")
                    modeRow(m.name, via(m) + depends, mode: m.mode)
                }
                ForEach(entry.groups, id: \.group) { g in
                    RowDivider(inset: 14)
                    modeRow("IdP group \(g.group)", rule(g.ruleOn), mode: g.mode)
                }
            } else {
                ProgressView().frame(maxWidth: .infinity).padding(16)
            }
        }
        if let access, !access.rulesReadable {
            Text("Only people who manage access can read the access rules, so this shows vault roles.")
                .font(.footnote)
                .foregroundStyle(.secondary)
                .padding(.horizontal, 16)
        }
    }

    private func via(_ m: MemberAccessCard) -> String {
        switch m.via {
        case "admin": "Team owner or admin"
        case "vault": "Vault role"
        default: rule(m.ruleOn)
        }
    }

    private func rule(_ on: String?) -> String {
        guard let on else { return "Vault role" }
        return on == host.hostId ? "Rule on this host" : "Rule on group \(labelOf(on))"
    }

    private func fact(_ title: String, _ subtitle: String, color: Color = .primary) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title).foregroundStyle(color)
            Text(subtitle).font(.footnote).foregroundStyle(.secondary)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func modeRow(_ title: String, _ subtitle: String, mode: String) -> some View {
        HStack {
            VStack(alignment: .leading, spacing: 2) {
                Text(title).lineLimit(1)
                Text(subtitle).font(.footnote).foregroundStyle(.secondary).lineLimit(2)
            }
            Spacer(minLength: 8)
            Text(modeText(mode)).foregroundStyle(mode == "none" ? Color.secondary : Color.accentColor)
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
    }
}

private func modeText(_ mode: String) -> String {
    switch mode {
    case "none": "Hidden"
    case "connect": "Connect only"
    case "read": "Read"
    default: "Edit"
    }
}

private func loginText(_ h: HostPolicyCard) -> String {
    let kind = switch h.auth.kind {
    case "opkssh": "OpenID (opkssh)"
    case "ssh_id": "SSH ID"
    case "certificate": "Certificate"
    case "key": "Key"
    case "password": "Password"
    default: "Asks when connecting"
    }
    return [h.auth.username, kind, h.auth.label].filter { !$0.isEmpty }.joined(separator: " · ")
}

private func networkText(_ h: HostPolicyCard) -> String {
    h.network?.label ?? "Direct, no VPN network"
}

private func jumpText(_ h: HostPolicyCard) -> String {
    h.jumpHosts.isEmpty ? "No jump host" : h.jumpHosts.map { $0.outsideVault ? "Outside this vault" : $0.label }.joined(separator: " → ")
}

private func sourceText(_ s: PolicySourceCard?) -> String {
    guard let s else { return "not set" }
    switch s.kind {
    case "host": return "set on the host"
    case "group": return "from group \(s.label)"
    case "tag": return "from tag \(s.label)"
    default: return "from your credentials \(s.label)"
    }
}

private func riskText(_ r: PolicyRiskCard) -> String {
    switch r.kind {
    case "bypasses_vpn": "Other hosts in its group use a VPN network; this one connects without one."
    case "password_auth": "Logs in with a password while the rest of the vault uses keys."
    case "password_auth_policy": "Logs in with a password, but the team allows keys only."
    case "jump_outside_vault": "A jump host is in another vault or was deleted, so connections fail."
    case "direct_connection_policy": "The team requires a jump host, and this host has none."
    case "overlay_replaces_identity": "Your own credentials replace the team's login on this device."
    default: "Its settings can't be read: \(r.detail)"
    }
}
