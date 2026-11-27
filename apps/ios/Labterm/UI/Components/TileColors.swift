import SwiftUI
import UIKit

/// Tile colours for hosts and groups (`Host.color` / `Group.color`, synced as
/// `#rrggbb`). Same palette as the desktop tag palette
/// (`apps/desktop/src/hosts/TagChip.tsx`) and Android's `TileColors`.
enum TileColors {
    static let palette = [
        "#2bb884",
        "#5aa9e6",
        "#b07cf2",
        "#f25e61",
        "#f8aa4b",
        "#e3c04b",
        "#4bc8c1",
        "#ef7fb6",
        "#8d99ae",
    ]

    /// `#rrggbb` for `#3a7`, `33aa77` or ` #33AA77 `; nil when it isn't a hex colour.
    static func normalize(_ input: String?) -> String? {
        var hex = (input ?? "").trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if hex.hasPrefix("#") { hex.removeFirst() }
        guard hex.count == 3 || hex.count == 6, hex.allSatisfy(\.isHexDigit) else { return nil }
        if hex.count == 3 { hex = hex.map { "\($0)\($0)" }.joined() }
        return "#" + hex
    }

    /// SwiftUI colour of a stored value, or nil for none or a malformed one.
    static func color(_ value: String?) -> Color? {
        guard let hex = normalize(value), let rgb = UInt32(hex.dropFirst(), radix: 16) else { return nil }
        return Color(
            red: Double((rgb >> 16) & 0xFF) / 255,
            green: Double((rgb >> 8) & 0xFF) / 255,
            blue: Double(rgb & 0xFF) / 255
        )
    }

    /// `#rrggbb` of a colour from the system colour picker.
    static func hex(_ color: Color) -> String? {
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0, a: CGFloat = 0
        guard UIColor(color).getRed(&r, green: &g, blue: &b, alpha: &a) else { return nil }
        let clamp = { (v: CGFloat) in Int((min(max(v, 0), 1) * 255).rounded()) }
        return String(format: "#%02x%02x%02x", clamp(r), clamp(g), clamp(b))
    }
}
