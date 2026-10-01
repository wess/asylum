// Renders the Asylum app icon — a friendly Bot face (rounded head, two eyes,
// an antenna) on a deep violet squircle — to a 1024x1024 PNG. Pure
// CoreGraphics. scripts/icon.sh turns the PNG into the .iconset/.icns.
// Usage: swift scripts/icon.swift out.png
import CoreGraphics
import Foundation
import ImageIO

let outPath = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "assets/icon.png"
let dim = 1024
let space = CGColorSpaceCreateDeviceRGB()

func color(_ r: Double, _ g: Double, _ b: Double, _ a: Double = 1) -> CGColor {
  CGColor(colorSpace: space, components: [r, g, b, a])!
}

guard let ctx = CGContext(
  data: nil, width: dim, height: dim, bitsPerComponent: 8, bytesPerRow: 0,
  space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
) else { fatalError("could not create context") }

let full = CGFloat(dim)
ctx.clear(CGRect(x: 0, y: 0, width: full, height: full))

let margin: CGFloat = 88
let body = CGRect(x: margin, y: margin, width: full - margin * 2, height: full - margin * 2)
let radius = body.width * 0.2237
let squircle = CGPath(roundedRect: body, cornerWidth: radius, cornerHeight: radius, transform: nil)

ctx.saveGState()
ctx.addPath(squircle)
ctx.clip()
let grad = CGGradient(
  colorsSpace: space,
  colors: [color(0.43, 0.30, 0.98), color(0.16, 0.08, 0.40)] as CFArray,
  locations: [0, 1]
)!
ctx.drawLinearGradient(grad, start: CGPoint(x: 0, y: full), end: CGPoint(x: full, y: 0), options: [])
let sheen = CGGradient(
  colorsSpace: space,
  colors: [color(1, 1, 1, 0.16), color(1, 1, 1, 0)] as CFArray,
  locations: [0, 1]
)!
ctx.drawRadialGradient(
  sheen,
  startCenter: CGPoint(x: full * 0.35, y: full * 0.85), startRadius: 0,
  endCenter: CGPoint(x: full * 0.35, y: full * 0.85), endRadius: full * 0.7,
  options: []
)
ctx.restoreGState()

// Head.
let head = CGRect(x: full * 0.25, y: full * 0.24, width: full * 0.5, height: full * 0.42)
let headPath = CGPath(roundedRect: head, cornerWidth: full * 0.12, cornerHeight: full * 0.12, transform: nil)
ctx.setShadow(offset: CGSize(width: 0, height: -14), blur: 40, color: color(0, 0, 0, 0.35))
ctx.addPath(headPath)
ctx.setFillColor(color(0.97, 0.96, 1.0))
ctx.fillPath()
ctx.setShadow(offset: .zero, blur: 0, color: nil)

// Antenna.
ctx.setStrokeColor(color(0.97, 0.96, 1.0))
ctx.setLineWidth(full * 0.028)
ctx.setLineCap(.round)
ctx.move(to: CGPoint(x: full * 0.5, y: head.maxY))
ctx.addLine(to: CGPoint(x: full * 0.5, y: head.maxY + full * 0.08))
ctx.strokePath()
ctx.setFillColor(color(1.0, 0.72, 0.30))
let bulb = full * 0.055
ctx.fillEllipse(in: CGRect(x: full * 0.5 - bulb, y: head.maxY + full * 0.07, width: bulb * 2, height: bulb * 2))

// Eyes.
ctx.setFillColor(color(0.16, 0.08, 0.40))
let eyeW = full * 0.07, eyeH = full * 0.11
for x in [full * 0.39, full * 0.61] {
  let r = CGRect(x: x - eyeW / 2, y: head.midY - eyeH / 2 + full * 0.02, width: eyeW, height: eyeH)
  ctx.addPath(CGPath(roundedRect: r, cornerWidth: eyeW / 2, cornerHeight: eyeW / 2, transform: nil))
  ctx.fillPath()
}

// Smile.
ctx.setStrokeColor(color(0.16, 0.08, 0.40))
ctx.setLineWidth(full * 0.022)
ctx.addArc(center: CGPoint(x: full * 0.5, y: head.midY - full * 0.02), radius: full * 0.07,
           startAngle: .pi * 1.2, endAngle: .pi * 1.8, clockwise: false)
ctx.strokePath()

guard let image = ctx.makeImage() else { fatalError("no image") }
let url = URL(fileURLWithPath: outPath)
guard let dest = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil) else { fatalError("no dest") }
CGImageDestinationAddImage(dest, image, nil)
CGImageDestinationFinalize(dest)
