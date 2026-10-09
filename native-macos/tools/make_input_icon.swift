// Render the input-source badge using a system font; no font files are bundled.
import AppKit
import CoreText

let output = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
let size = NSSize(width: 20, height: 16)
let font = NSFont(name: "PingFangSC-Semibold", size: 13.5)!
let line = CTLineCreateWithAttributedString(
  NSAttributedString(string: "果", attributes: [.font: font]))
let glyphPath = CGMutablePath()
for run in CTLineGetGlyphRuns(line) as! [CTRun] {
  let count = CTRunGetGlyphCount(run)
  var glyphs = [CGGlyph](repeating: 0, count: count)
  var positions = [CGPoint](repeating: .zero, count: count)
  CTRunGetGlyphs(run, CFRange(location: 0, length: count), &glyphs)
  CTRunGetPositions(run, CFRange(location: 0, length: count), &positions)
  let runFont = (CTRunGetAttributes(run) as NSDictionary)[kCTFontAttributeName] as! CTFont
  for index in 0..<count {
    if let path = CTFontCreatePathForGlyph(runFont, glyphs[index], nil) {
      glyphPath.addPath(
        path, transform: CGAffineTransform(translationX: positions[index].x, y: positions[index].y))
    }
  }
}
let ink = glyphPath.boundingBoxOfPath
precondition(!ink.isEmpty, "The system font must provide the Chinese glyph.")
// Center visible ink, rather than the font's advance width or line height.
var placement = CGAffineTransform(
  translationX: (size.width - ink.width) / 2 - ink.minX,
  y: (size.height - ink.height) / 2 - ink.minY)
let mark = glyphPath.copy(using: &placement)!

for (name, white) in [("GuoTemplate", false), ("GuoSelected", true)] {
  var representations: [NSBitmapImageRep] = []
  for scale in [1, 2] {
    let bitmap = NSBitmapImageRep(
      bitmapDataPlanes: nil, pixelsWide: 20 * scale, pixelsHigh: 16 * scale,
      bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
      colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    bitmap.size = size
    let graphics = NSGraphicsContext(bitmapImageRep: bitmap)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = graphics
    let context = graphics.cgContext
    context.setFillColor((white ? NSColor.white : NSColor.black).cgColor)
    context.addPath(
      CGPath(
        roundedRect: CGRect(origin: .zero, size: size), cornerWidth: 3.5, cornerHeight: 3.5,
        transform: nil))
    context.fillPath()
    // Transparent lettering lets the badge work against native menu surfaces.
    context.setBlendMode(.clear)
    context.addPath(mark)
    context.fillPath()
    NSGraphicsContext.restoreGraphicsState()
    representations.append(bitmap)
  }
  let image = NSImage(size: size)
  for bitmap in representations { image.addRepresentation(bitmap) }
  try image.tiffRepresentation!.write(to: output.appendingPathComponent(name + ".tiff"))
  if !white {
    try representations[1].representation(using: .png, properties: [:])!.write(
      to: output.appendingPathComponent("GuoPreview.png"))
  }
}
print("Rendered 果 input-source icons at 1× and 2×.")
