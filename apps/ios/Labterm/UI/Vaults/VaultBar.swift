import SwiftUI
import LabtermCore

/// Display name of a vault: the own vaults are always "Personal" and "Local".
func vaultDisplayName(_ vault: VaultInfo) -> String {
    switch vault.kind {
    case .personal: "Personal"
    case .local: "Local"
    case .team: vault.name
    }
}

/// A vault's tile. The kind is a corner badge (person = Personal, laptop =
/// Local, none = team), never the icon or colour, which every vault may set.
/// Pinned: filled with a pin; unpinned: outlined.
struct VaultTile: View {
    let vault: VaultInfo
    let pinned: Bool
    let active: Bool
    var size: CGFloat = 40

    private var color: Color {
        if let own = TileColors.color(vault.color) { return own }
        var h: UInt32 = 0
        for u in vault.id.unicodeScalars { h = h &* 31 &+ u.value }
        return TileColors.color(TileColors.palette[Int(h % UInt32(TileColors.palette.count))]) ?? .gray
    }

    private var initials: String {
        let words = vaultDisplayName(vault).split(separator: " ")
        let s = words.count > 1 ? "\(words[0].prefix(1))\(words[1].prefix(1))" : String((words.first ?? "?").prefix(2))
        return s.uppercased()
    }

    var body: some View {
        let shape = RoundedRectangle(cornerRadius: size * 0.28, style: .continuous)
        ZStack {
            Group {
                if pinned { shape.fill(color) } else { shape.strokeBorder(color, lineWidth: 1.5) }
            }
            .frame(width: size, height: size)
            if vault.locked {
                Image(systemName: "lock.fill").foregroundStyle(pinned ? .white : color)
            } else if let custom = CustomIcon.image(vault.icon) {
                Image(uiImage: custom)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(width: size * 0.65, height: size * 0.65)
            } else if let glyph = DistroIcons.picked(vault.icon) {
                glyph.image
                    .resizable()
                    .renderingMode(.template)
                    .aspectRatio(contentMode: .fit)
                    .foregroundStyle(pinned ? .white : color)
                    .frame(width: size * 0.55, height: size * 0.55)
            } else {
                Text(initials)
                    .font(.system(size: size * 0.34, weight: .bold))
                    .foregroundStyle(pinned ? .white : color)
            }
        }
        .padding(3)
        .overlay {
            if active {
                RoundedRectangle(cornerRadius: size * 0.34, style: .continuous).stroke(.primary, lineWidth: 2)
            }
        }
        .overlay(alignment: .bottomTrailing) {
            if let badge = kindBadge {
                Image(systemName: badge)
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(Color(.systemBackground))
                    .frame(width: 18, height: 18)
                    .background(Circle().fill(.primary))
                    .overlay(Circle().stroke(Color(.systemBackground), lineWidth: 2))
                    .offset(x: 3, y: 3)
            }
        }
        .overlay(alignment: .topTrailing) {
            if pinned {
                Image(systemName: "pin.fill")
                    .font(.system(size: 10))
                    .rotationEffect(.degrees(30))
                    .offset(x: 3, y: -3)
            }
        }
        .accessibilityHidden(true)
    }

    private var kindBadge: String? {
        switch vault.kind {
        case .personal: "person.fill"
        case .local: "laptopcomputer"
        case .team: nil
        }
    }
}

/// Full-width vault bar: pinned vaults first as filled tiles, a divider, the
/// rest outlined. Tap switches the vault; long-press pins or unpins.
struct VaultBar: View {
    @Bindable var model: VaultsModel

    var body: some View {
        if model.vaults.count > 1 {
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(alignment: .bottom, spacing: 4) {
                    ForEach(model.pinnedVaults, id: \.id) { item($0, pinned: true) }
                    if !model.pinnedVaults.isEmpty && !model.unpinnedVaults.isEmpty {
                        Divider().frame(height: 40).padding(.horizontal, 4)
                    }
                    ForEach(model.unpinnedVaults, id: \.id) { item($0, pinned: false) }
                }
                .padding(.horizontal, 8)
                .padding(.vertical, 8)
            }
            .background(.bar)
            .accessibilityIdentifier("vaults.bar")
        }
    }

    private func item(_ vault: VaultInfo, pinned: Bool) -> some View {
        let active = vault.id == model.selectedVaultId
        return Button {
            model.selectedVaultId = vault.id
        } label: {
            VStack(spacing: 2) {
                VaultTile(vault: vault, pinned: pinned, active: active, size: pinned ? 40 : 34)
                Text(vaultDisplayName(vault))
                    .font(.caption2.weight(active ? .semibold : .regular))
                    .foregroundStyle(active ? .primary : .secondary)
                    .lineLimit(1)
            }
            .frame(width: 64)
        }
        .buttonStyle(.plain)
        .disabled(vault.locked)
        .contextMenu {
            Button(pinned ? "Unpin vault" : "Pin vault", systemImage: pinned ? "pin.slash" : "pin") {
                model.togglePin(vault.id)
            }
        }
        .accessibilityLabel(vaultDisplayName(vault))
        .accessibilityAddTraits(active ? .isSelected : [])
    }
}
