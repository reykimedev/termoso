import SwiftUI
import LabtermCore

/// Rounded-square tile shared by hosts and groups: a custom image, a distro
/// glyph or an SF Symbol, tinted with `tint` on a light wash of it.
struct IconTileView: View {
    let custom: UIImage?
    let icon: DistroIcon?
    let symbol: String
    let tint: Color
    var size: CGFloat = 44

    var body: some View {
        ZStack {
            RoundedRectangle(cornerRadius: size * 0.28, style: .continuous)
                .fill(tint.opacity(0.14))
            if let custom {
                Image(uiImage: custom)
                    .resizable()
                    .interpolation(.high)
                    .aspectRatio(contentMode: .fit)
                    .padding(size * 0.16)
            } else if let icon {
                icon.image
                    .resizable()
                    .renderingMode(.template)
                    .aspectRatio(contentMode: .fit)
                    .foregroundStyle(tint)
                    .padding(size * 0.22)
            } else {
                Image(systemName: symbol)
                    .font(.system(size: size * 0.42, weight: .semibold))
                    .foregroundStyle(tint)
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

/// Large rounded-square host tile: a custom image or the distro glyph in its
/// brand colour on a tinted background, or a generic server symbol until the
/// OS is known. `icon` (manual override) wins over the detected `osName`;
/// `color` (`#rrggbb`) wins over the brand colour.
struct HostAvatar: View {
    let icon: String?
    let osName: String?
    let color: String?
    let symbol: String
    var size: CGFloat = 44

    init(host: HostItem, size: CGFloat = 44) {
        icon = host.icon
        osName = host.osName
        color = host.color
        symbol = switch host.protocol {
        case "telnet": "network"
        case "serial": "cable.connector"
        default: host.useMosh ? "antenna.radiowaves.left.and.right" : "server.rack"
        }
        self.size = size
    }

    init(icon: String?, osName: String?, color: String?, size: CGFloat = 44) {
        self.icon = icon
        self.osName = osName
        self.color = color
        symbol = "server.rack"
        self.size = size
    }

    var body: some View {
        let glyph = DistroIcons.picked(icon) ?? DistroIcons.forOs(osName)
        IconTileView(
            custom: CustomIcon.image(icon),
            icon: glyph,
            symbol: symbol,
            tint: TileColors.color(color) ?? glyph?.color ?? Color.accentColor,
            size: size
        )
    }
}

/// Group tile: its custom image or picked glyph, else the folder, tinted with
/// the group's colour (or the accent).
struct GroupAvatar: View {
    var icon: String? = nil
    var color: String? = nil
    var size: CGFloat = 44

    var body: some View {
        let glyph = DistroIcons.picked(icon)
        IconTileView(
            custom: CustomIcon.image(icon),
            icon: glyph,
            symbol: "folder.fill",
            tint: TileColors.color(color) ?? glyph?.color ?? Color.accentColor,
            size: size
        )
    }
}
