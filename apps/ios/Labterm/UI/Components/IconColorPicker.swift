import PhotosUI
import SwiftUI

extension DistroIcons {
    /// Icon for a stored pick (`Host.icon` / `Group.icon`). Custom images are
    /// not ids: matching their base64 against OS names would pick a random logo.
    static func picked(_ id: String?) -> DistroIcon? {
        guard let id, !id.hasPrefix("custom:") else { return nil }
        return forOs(id)
    }
}

/// Icon and colour for a host or group: automatic, a custom photo (scaled to
/// a 64 px PNG), the OS logos, and the tile colour (palette, system picker or
/// none). Writes straight into the bindings; the editor saves.
struct IconColorPicker: View {
    @Binding var icon: String?
    @Binding var color: String?
    let automaticLabel: String
    let preview: (String?, String?) -> AnyView

    @State private var photo: PhotosPickerItem?
    @State private var photoError = false

    private let columns = [GridItem(.adaptive(minimum: 64), spacing: 12)]

    var body: some View {
        Form {
            Section {
                HStack {
                    Spacer()
                    preview(icon, color)
                    Spacer()
                }
            }
            Section("Colour") {
                LazyVGrid(columns: columns, spacing: 12) {
                    swatch(nil)
                    ForEach(TileColors.palette, id: \.self) { swatch($0) }
                }
                .padding(.vertical, 4)
                ColorPicker(
                    "Custom colour",
                    selection: Binding(
                        get: { TileColors.color(color) ?? .gray },
                        set: { color = TileColors.hex($0) }
                    ),
                    supportsOpacity: false
                )
                .accessibilityIdentifier("iconPicker.customColour")
            }
            Section("Icon") {
                Button {
                    icon = nil
                } label: {
                    Label(automaticLabel, systemImage: icon == nil ? "checkmark.circle.fill" : "sparkles")
                }
                PhotosPicker(selection: $photo, matching: .images) {
                    Label(
                        "Custom image…",
                        systemImage: CustomIcon.isCustom(icon) ? "checkmark.circle.fill" : "photo.badge.plus"
                    )
                }
                if photoError {
                    Text("This image can't be used. Pick a PNG, JPEG or HEIC photo.")
                        .font(.footnote)
                        .foregroundStyle(.red)
                }
                LazyVGrid(columns: columns, spacing: 12) {
                    ForEach(DistroIcons.all) { i in
                        Button {
                            icon = i.id
                        } label: {
                            VStack(spacing: 4) {
                                IconTileView(custom: nil, icon: i, symbol: "", tint: i.color, size: 44)
                                    .overlay(
                                        RoundedRectangle(cornerRadius: 12, style: .continuous)
                                            .stroke(Color.accentColor, lineWidth: icon == i.id ? 2 : 0)
                                    )
                                Text(i.title)
                                    .font(.caption2)
                                    .lineLimit(1)
                                    .foregroundStyle(.primary)
                            }
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel(i.title)
                    }
                }
                .padding(.vertical, 4)
            }
        }
        .navigationTitle("Icon and colour")
        .navigationBarTitleDisplayMode(.inline)
        .onChange(of: photo) { _, item in
            guard let item else { return }
            Task {
                let data = try? await item.loadTransferable(type: Data.self)
                await MainActor.run {
                    if let data, let value = CustomIcon.encode(data) {
                        icon = value
                        photoError = false
                    } else {
                        photoError = true
                    }
                    photo = nil
                }
            }
        }
    }

    private func swatch(_ hex: String?) -> some View {
        let on = TileColors.normalize(color) == hex
        return Button {
            color = hex
        } label: {
            ZStack {
                Circle()
                    .fill(TileColors.color(hex) ?? Color.clear)
                    .overlay(Circle().stroke(Color.secondary.opacity(hex == nil ? 0.5 : 0), lineWidth: 1))
                    .frame(width: 32, height: 32)
                if hex == nil {
                    Image(systemName: "nosign").foregroundStyle(.secondary)
                } else if on {
                    Image(systemName: "checkmark").font(.footnote.weight(.bold)).foregroundStyle(.white)
                }
            }
            .overlay(Circle().stroke(Color.accentColor, lineWidth: on ? 2 : 0).frame(width: 38, height: 38))
        }
        .buttonStyle(.plain)
        .accessibilityLabel(hex ?? "No colour")
    }
}
