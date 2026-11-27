import SwiftUI
import LabtermCore

/// Vault tile: the vault's colour and icon (synced next to its name, or set
/// on this device for the local vault), falling back to the kind's symbol; a
/// small person / phone badge tells personal and local vaults apart, and a
/// locked vault shows a dimmed lock.
struct VaultAvatar: View {
    let vault: VaultInfo
    var size: CGFloat = 44

    static func symbol(_ kind: VaultKind) -> String {
        switch kind {
        case .local: "iphone"
        case .personal: "person.fill"
        case .team: "person.3.fill"
        }
    }

    var body: some View {
        let glyph = DistroIcons.picked(vault.icon)
        ZStack(alignment: .bottomTrailing) {
            if vault.locked {
                IconTileView(custom: nil, icon: nil, symbol: "lock.fill", tint: .secondary, size: size)
                    .opacity(0.55)
            } else {
                IconTileView(
                    custom: CustomIcon.image(vault.icon),
                    icon: glyph,
                    symbol: Self.symbol(vault.kind),
                    tint: TileColors.color(vault.color) ?? glyph?.color ?? Color.accentColor,
                    size: size
                )
            }
            if vault.kind != .team {
                Image(systemName: Self.symbol(vault.kind))
                    .font(.system(size: size * 0.2, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .frame(width: size * 0.42, height: size * 0.42)
                    .background(Circle().fill(Color(.secondarySystemBackground)))
                    .overlay(Circle().stroke(Color.secondary.opacity(0.3), lineWidth: 1))
                    .offset(x: 2, y: 2)
            }
        }
        .accessibilityHidden(true)
    }
}
