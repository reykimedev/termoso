import SwiftUI
import LabtermCore

/// Hosts and sub-groups of one group (nil = vault root) with search and a
/// tag filter. Tapping a host connects; long-press (or the pencil) edits.
struct HostsView: View {
    @Bindable var model: VaultsModel
    let groupId: String?

    @Environment(SessionStore.self) private var sessions
    @State private var search = ""
    @State private var tagFilter: String?
    @State private var editor: HostEditorTarget?
    @State private var newGroupName: String?
    @State private var alert: String?
    /// A big group's "Connect all" waiting for a yes.
    @State private var confirmConnect: (label: String, hosts: [HostItem])?

    /// Opening more sessions than this at once asks first.
    private static let connectAllConfirmAbove = 10

    private var title: String {
        groupId.flatMap { model.group($0)?.label } ?? "Hosts"
    }

    var body: some View {
        ScrollView {
            VStack(spacing: 16) {
                SearchField(prompt: "Search hosts", text: $search, identifier: "hosts.search")
                if let tagFilter {
                    HStack {
                        Button {
                            self.tagFilter = nil
                        } label: {
                            Label(tagFilter, systemImage: "xmark")
                                .font(.footnote.weight(.medium))
                                .padding(.horizontal, 10)
                                .padding(.vertical, 6)
                                .background(Color.accentColor.opacity(0.14), in: Capsule())
                        }
                        .buttonStyle(.plain)
                        Spacer()
                    }
                }
                if search.isEmpty {
                    let groups = model.groups(inGroup: groupId)
                    if !groups.isEmpty {
                        CardSection("Groups") {
                            ForEach(Array(groups.enumerated()), id: \.element.id) { index, group in
                                if index > 0 { RowDivider() }
                                NavigationLink {
                                    HostsView(model: model, groupId: group.id)
                                } label: {
                                    GroupRow(group: group)
                                }
                                .buttonStyle(.plain)
                                .accessibilityIdentifier("hosts.group.\(group.label)")
                                .contextMenu {
                                    Button("Connect all", systemImage: "play.fill") { connectAll(group.id, label: group.label) }
                                    Button("Delete group", systemImage: "trash", role: .destructive) { model.deleteGroup(id: group.id) }
                                }
                            }
                        }
                    }
                }

                let hosts = model.hosts(inGroup: groupId, matching: search, tag: tagFilter)
                CardSection(search.isEmpty ? "Hosts" : "Results") {
                    if hosts.isEmpty {
                        emptyState
                    } else {
                        ForEach(Array(hosts.enumerated()), id: \.element.id) { index, host in
                            if index > 0 { RowDivider() }
                            Button {
                                sessions.connect(host: host)
                            } label: {
                                HostRow(host: host, showPath: !search.isEmpty) {
                                    editor = .edit(id: host.id)
                                }
                            }
                            .buttonStyle(.plain)
                            .accessibilityIdentifier("hosts.host.\(host.label)")
                            .contextMenu {
                                hostMenu(host)
                            }
                        }
                    }
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 8)
        }
        .pageBackground()
        .navigationTitle(title)
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                if let groupId {
                    Button("Connect all", systemImage: "play.fill") { connectAll(groupId, label: title) }
                        .accessibilityIdentifier("hosts.connectAll")
                }
                tagMenu
                addMenu
            }
        }
        .sheet(item: $editor) { target in
            HostEditorView(model: model, target: target)
        }
        .alert("New group", isPresented: Binding(
            get: { newGroupName != nil },
            set: { if !$0 { newGroupName = nil } }
        )) {
            TextField("Name", text: Binding(get: { newGroupName ?? "" }, set: { newGroupName = $0 }))
                .accessibilityIdentifier("hosts.newGroup.name")
            Button("Cancel", role: .cancel) { newGroupName = nil }
            Button("Create") {
                let name = (newGroupName ?? "").trimmingCharacters(in: .whitespaces)
                if !name.isEmpty { model.createGroup(label: name, parentId: groupId) }
                newGroupName = nil
            }
            .accessibilityIdentifier("hosts.newGroup.create")
        }
        .alert(
            "Connect \(confirmConnect?.hosts.count ?? 0) hosts?",
            isPresented: Binding(get: { confirmConnect != nil }, set: { if !$0 { confirmConnect = nil } })
        ) {
            Button("Cancel", role: .cancel) { confirmConnect = nil }
            Button("Connect all") {
                if let c = confirmConnect { c.hosts.forEach { sessions.connect(host: $0, tabGroup: c.label) } }
                confirmConnect = nil
            }
        } message: {
            Text("Opens a session for every host in “\(confirmConnect?.label ?? "")” and its sub-groups.")
        }
        .alert("Error", isPresented: Binding(get: { alert != nil }, set: { if !$0 { alert = nil } })) {
            Button("OK", role: .cancel) { alert = nil }
        } message: {
            Text(alert ?? "")
        }
        .onChange(of: model.error) { _, newValue in
            if let newValue { alert = newValue }
        }
    }

    /// One session per terminal host in the group and its sub-groups.
    private func connectAll(_ id: String, label: String) {
        let hosts = model.connectAllHosts(inGroup: id)
        if hosts.isEmpty {
            alert = "No hosts to connect in “\(label)”."
        } else if hosts.count > Self.connectAllConfirmAbove {
            confirmConnect = (label, hosts)
        } else {
            hosts.forEach { sessions.connect(host: $0, tabGroup: label) }
        }
    }

    private var emptyState: some View {
        VStack(spacing: 8) {
            Image(systemName: search.isEmpty ? "server.rack" : "magnifyingglass")
                .font(.largeTitle)
                .foregroundStyle(.tertiary)
            Text(search.isEmpty ? "No hosts yet" : "Nothing matches “\(search)”")
                .font(.headline)
            if search.isEmpty {
                Text("Add a server with the + button. Everything is stored encrypted on this device.")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
        .padding(.horizontal, 16)
        .accessibilityIdentifier("hosts.empty")
    }

    @ViewBuilder
    private func hostMenu(_ host: HostItem) -> some View {
        Button("Connect", systemImage: "terminal") { sessions.connect(host: host) }
            .accessibilityIdentifier("hosts.menu.connect")
        if host.telnetPort != nil {
            Button("Connect with Telnet", systemImage: "network") { sessions.connect(host: host, transport: .telnet) }
        }
        if host.useMosh {
            Button("Connect with Mosh", systemImage: "antenna.radiowaves.left.and.right") { sessions.connect(host: host, transport: .mosh) }
        }
        Divider()
        Button("Edit", systemImage: "pencil") { editor = .edit(id: host.id) }
            .accessibilityIdentifier("hosts.menu.edit")
        Button("Delete", systemImage: "trash", role: .destructive) { model.deleteHost(id: host.id) }
            .accessibilityIdentifier("hosts.menu.delete")
    }

    private var addMenu: some View {
        Menu {
            Button {
                editor = .new(groupId: groupId)
            } label: {
                Label("New host", systemImage: "plus.rectangle.on.rectangle")
            }
            .accessibilityIdentifier("hosts.newHost")
            Button {
                newGroupName = ""
            } label: {
                Label("New group", systemImage: "folder.badge.plus")
            }
            .accessibilityIdentifier("hosts.newGroupButton")
        } label: {
            Image(systemName: "plus")
        }
        .accessibilityIdentifier("hosts.add")
    }

    @ViewBuilder
    private var tagMenu: some View {
        if !model.tags.isEmpty {
            Menu {
                Button {
                    tagFilter = nil
                } label: {
                    Label("All tags", systemImage: tagFilter == nil ? "checkmark" : "")
                }
                Divider()
                ForEach(model.tags, id: \.id) { tag in
                    Button {
                        tagFilter = tagFilter == tag.label ? nil : tag.label
                    } label: {
                        Label("\(tag.label) (\(tag.hosts))", systemImage: tagFilter == tag.label ? "checkmark" : "")
                    }
                }
            } label: {
                Image(systemName: tagFilter == nil ? "tag" : "tag.fill")
            }
            .accessibilityIdentifier("hosts.tagFilter")
        }
    }
}

struct GroupRow: View {
    let group: GroupItem

    var body: some View {
        CardRow(title: group.label, subtitle: summary, chevron: true) {
            GroupAvatar()
        }
    }

    private var summary: String {
        var parts: [String] = []
        if group.hostCount > 0 { parts.append("\(group.hostCount) host\(group.hostCount == 1 ? "" : "s")") }
        if group.groupCount > 0 { parts.append("\(group.groupCount) group\(group.groupCount == 1 ? "" : "s")") }
        if group.hasConfig { parts.append("shared settings") }
        return parts.isEmpty ? "Empty" : parts.joined(separator: " · ")
    }
}

struct HostRow: View {
    let host: HostItem
    var showPath = false
    var onEdit: (() -> Void)? = nil

    var body: some View {
        HStack(spacing: 12) {
            HostAvatar(host: host)
            VStack(alignment: .leading, spacing: 2) {
                Text(host.label.isEmpty ? host.address : host.label)
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                Text(subtitle)
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                if !host.tags.isEmpty {
                    Text(host.tags.joined(separator: " · "))
                        .font(.caption2)
                        .foregroundStyle(Color.accentColor)
                        .lineLimit(1)
                }
                HostBadgesRow(host: host)
            }
            Spacer(minLength: 8)
            if let onEdit {
                Button(action: onEdit) {
                    Image(systemName: "pencil")
                        .font(.footnote.weight(.semibold))
                        .foregroundStyle(.secondary)
                        .frame(width: 32, height: 32)
                        .background(Theme.cardInset, in: Circle())
                }
                .buttonStyle(.plain)
                .accessibilityIdentifier("hosts.edit.\(host.label)")
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 10)
        .contentShape(Rectangle())
    }

    private var subtitle: String {
        var text = host.username.isEmpty ? host.address : "\(host.username)@\(host.address)"
        if host.port != 22 || host.protocol != "ssh" {
            text += ":\(host.port)"
        }
        if host.protocol != "ssh" {
            text += " · \(host.protocol)"
        }
        if showPath, !host.groupPath.isEmpty {
            text = host.groupPath.joined(separator: " / ") + " · " + text
        }
        return text
    }
}

/// VPN network and connection facts under a host row, the same set as Android and desktop.
/// iOS cannot join a VPN network yet, so the network chip has no tunnel state colour.
struct HostBadgesRow: View {
    let host: HostItem

    private struct Fact: Identifiable {
        let id: String
        let symbol: String
        let label: String
        var warn = false
    }

    private var facts: [Fact] {
        let b = host.badges
        var out: [Fact] = []
        switch b.auth {
        case "opkssh": out.append(Fact(id: "auth", symbol: "person.text.rectangle", label: "OpenID (opkssh)"))
        case "ssh_id": out.append(Fact(id: "auth", symbol: "touchid", label: "SSH ID"))
        case "certificate": out.append(Fact(id: "auth", symbol: "checkmark.shield", label: "Certificate"))
        case "key": out.append(Fact(id: "auth", symbol: "key", label: "SSH key"))
        case "password": out.append(Fact(id: "auth", symbol: "ellipsis.rectangle", label: "Password"))
        default: break
        }
        if b.overlayPaused {
            out.append(Fact(id: "overlay", symbol: "person.slash", label: "My credentials paused", warn: true))
        } else if b.overlay {
            out.append(Fact(id: "overlay", symbol: "person", label: "My credentials"))
        }
        if b.hostKeyTrusted == false {
            out.append(Fact(id: "hostKey", symbol: "exclamationmark.shield", label: "Server key not yet trusted", warn: true))
        }
        if b.jump { out.append(Fact(id: "jump", symbol: "point.3.connected.trianglepath.dotted", label: "Jump host")) }
        if b.gateway { out.append(Fact(id: "gateway", symbol: "wifi.router", label: "Gateway")) }
        if b.proxy { out.append(Fact(id: "proxy", symbol: "shield", label: "Proxy")) }
        if host.useMosh { out.append(Fact(id: "mosh", symbol: "bolt", label: "Mosh")) }
        return out
    }

    private func vpnLabel(_ vpn: HostVpnItem) -> String {
        vpn.inherited ? "VPN network \(vpn.label), from its group" : "VPN network \(vpn.label)"
    }

    var body: some View {
        let list = facts
        if host.vpn != nil || !list.isEmpty {
            HStack(spacing: 6) {
                if let vpn = host.vpn {
                    Label(vpn.label, systemImage: "lock.shield")
                        .font(.caption2.weight(.semibold))
                        .lineLimit(1)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(vpn.inherited ? Color.clear : Theme.cardInset, in: Capsule())
                        .overlay(Capsule().strokeBorder(vpn.inherited ? Color.secondary.opacity(0.5) : Color.clear))
                        .accessibilityLabel(vpnLabel(vpn))
                }
                ForEach(list) { f in
                    Image(systemName: f.symbol)
                        .font(.caption2)
                        .foregroundStyle(f.warn ? Color.orange : Color.secondary)
                        .accessibilityLabel(f.label)
                }
            }
            .padding(.top, 2)
        }
    }
}
