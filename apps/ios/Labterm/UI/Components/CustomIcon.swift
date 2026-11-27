import UIKit

/// Custom host/group icons: `Host.icon` / `Group.icon` hold
/// `custom:data:<data URL>`, a small PNG (or another raster) or an SVG of at
/// most 16 KiB, the whole value at most 32 KiB. Same rules as
/// `apps/desktop/src/hosts/customIcon.ts` and Android's `CustomIcon`.
/// UIKit has no SVG renderer, so SVG icons show the built-in tile here.
enum CustomIcon {
    private static let prefix = "custom:data:"
    private static let maxLength = 32 * 1024
    static let maxFileBytes = 8 * 1024 * 1024
    static let rasterSide: CGFloat = 64
    private static let mimes: Set<String> = [
        "image/png", "image/jpeg", "image/gif", "image/webp", "image/svg+xml", "image/x-icon", "image/avif",
    ]
    private static let cache = NSCache<NSString, UIImage>()

    private static func parse(_ value: String?) -> (mime: String, data: Data)? {
        guard let value, value.hasPrefix(prefix) else { return nil }
        let url = value.dropFirst(prefix.count)
        guard url.count <= maxLength, url.hasPrefix("data:"),
              let comma = url.firstIndex(of: ",") else { return nil }
        let header = url[url.index(url.startIndex, offsetBy: 5) ..< comma]
        guard header.hasSuffix(";base64") else { return nil }
        let mime = header.dropLast(";base64".count).lowercased()
        guard mimes.contains(mime), let data = Data(base64Encoded: String(url[url.index(after: comma)...])),
              !data.isEmpty else { return nil }
        return (mime, data)
    }

    static func isCustom(_ value: String?) -> Bool { parse(value) != nil }

    /// Decoded image, or nil for built-in ids, malformed values and SVGs.
    static func image(_ value: String?) -> UIImage? {
        guard let value, let parsed = parse(value), parsed.mime != "image/svg+xml" else { return nil }
        if let hit = cache.object(forKey: value as NSString) { return hit }
        guard let image = UIImage(data: parsed.data) else { return nil }
        cache.setObject(image, forKey: value as NSString)
        return image
    }

    /// Stored value for a picked photo: scaled into a transparent 64 px PNG.
    static func encode(_ data: Data) -> String? {
        guard !data.isEmpty, data.count <= maxFileBytes, let source = UIImage(data: data),
              source.size.width > 0, source.size.height > 0 else { return nil }
        let side = rasterSide
        let scale = min(side / source.size.width, side / source.size.height)
        let size = CGSize(width: source.size.width * scale, height: source.size.height * scale)
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = false
        let png = UIGraphicsImageRenderer(size: CGSize(width: side, height: side), format: format).pngData { _ in
            source.draw(in: CGRect(x: (side - size.width) / 2, y: (side - size.height) / 2, width: size.width, height: size.height))
        }
        let value = prefix + "data:image/png;base64," + png.base64EncodedString()
        return isCustom(value) ? value : nil
    }
}
