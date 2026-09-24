// Generates AppIcon.icns: a white wave on a cyan → purple gradient,
// within the macOS icon template (rounded "squircle" square).
import AppKit

func drawIcon(size: CGFloat) -> NSImage {
    let image = NSImage(size: NSSize(width: size, height: size))
    image.lockFocus()
    guard let context = NSGraphicsContext.current?.cgContext else { return image }

    // macOS template margin: the artwork occupies ~80% of the canvas.
    let inset = size * 0.10
    let rect = CGRect(x: inset, y: inset, width: size - inset * 2, height: size - inset * 2)
    let radius = rect.width * 0.2237  // Apple's "squircle" radius

    let path = CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil)
    context.saveGState()
    context.addPath(path)
    context.clip()

    let colors = [
        NSColor(srgbRed: 0.208, green: 0.765, blue: 0.878, alpha: 1).cgColor,
        NSColor(srgbRed: 0.545, green: 0.486, blue: 0.965, alpha: 1).cgColor,
    ]
    let gradient = CGGradient(
        colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: colors as CFArray, locations: [0, 1])!
    context.drawLinearGradient(
        gradient, start: CGPoint(x: rect.minX, y: rect.maxY),
        end: CGPoint(x: rect.maxX, y: rect.minY), options: [])
    context.restoreGState()

    // The wave: a continuous line, the app's signature.
    let width = rect.width
    let midY = rect.midY
    let wave = CGMutablePath()
    let points: [(CGFloat, CGFloat)] = [
        (0.15, 0.0), (0.29, 0.12), (0.42, -0.20), (0.50, 0.24),
        (0.58, -0.20), (0.71, 0.12), (0.85, 0.0),
    ]
    wave.move(to: CGPoint(x: rect.minX + width * points[0].0, y: midY))
    for index in 1..<points.count {
        let previous = points[index - 1]
        let current = points[index]
        let controlX = rect.minX + width * (previous.0 + current.0) / 2
        wave.addCurve(
            to: CGPoint(x: rect.minX + width * current.0, y: midY + rect.height * current.1),
            control1: CGPoint(x: controlX, y: midY + rect.height * previous.1),
            control2: CGPoint(x: controlX, y: midY + rect.height * current.1))
    }
    context.setStrokeColor(NSColor.white.cgColor)
    context.setLineWidth(size * 0.055)
    context.setLineCap(.round)
    context.setLineJoin(.round)
    context.addPath(wave)
    context.strokePath()

    image.unlockFocus()
    return image
}

let iconset = URL(fileURLWithPath: CommandLine.arguments[1])
try? FileManager.default.createDirectory(at: iconset, withIntermediateDirectories: true)

for (base, scale) in [(16, 1), (16, 2), (32, 1), (32, 2), (128, 1), (128, 2),
                      (256, 1), (256, 2), (512, 1), (512, 2)] {
    let pixels = CGFloat(base * scale)
    let image = drawIcon(size: pixels)
    guard let tiff = image.tiffRepresentation,
          let rep = NSBitmapImageRep(data: tiff),
          let png = rep.representation(using: .png, properties: [:])
    else { continue }
    let name = scale == 1 ? "icon_\(base)x\(base).png" : "icon_\(base)x\(base)@2x.png"
    try png.write(to: iconset.appending(path: name))
}
print("iconset écrit dans \(iconset.path)")
