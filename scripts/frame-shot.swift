// 창 스크린샷을 밝은 배경 위에 여백·그림자와 함께 올림(문서·릴리스 노트용).
// 사용: swift scripts/frame-shot.swift <입력.png> <출력.png> [여백=72] [배경=F7F8FA]
import AppKit

let args = CommandLine.arguments
guard args.count >= 3, let src = NSImage(contentsOfFile: args[1]),
      let rep = src.representations.first else {
    FileHandle.standardError.write("usage: frame-shot <in.png> <out.png> [pad] [hex]\n".data(using: .utf8)!)
    exit(1)
}
let pad = CGFloat(args.count > 3 ? Double(args[3]) ?? 72 : 72)
let hex = args.count > 4 ? args[4] : "F7F8FA"
func color(_ h: String) -> NSColor {
    var v: UInt64 = 0
    Scanner(string: h).scanHexInt64(&v)
    return NSColor(srgbRed: CGFloat((v >> 16) & 0xFF) / 255, green: CGFloat((v >> 8) & 0xFF) / 255,
                   blue: CGFloat(v & 0xFF) / 255, alpha: 1)
}
// 픽셀 크기 그대로(레티나 2배) 작업함.
let w = CGFloat(rep.pixelsWide), h = CGFloat(rep.pixelsHigh)
let scale = w / src.size.width
let p = pad * scale
let size = NSSize(width: w + p * 2, height: h + p * 2)
guard let out = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(size.width), pixelsHigh: Int(size.height),
                                 bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                                 colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0) else { exit(1) }
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: out)
color(hex).setFill()
NSRect(origin: .zero, size: size).fill()
let shadow = NSShadow()
shadow.shadowColor = NSColor(white: 0, alpha: 0.16)
shadow.shadowBlurRadius = 28 * scale
shadow.shadowOffset = NSSize(width: 0, height: -10 * scale)
shadow.set()
src.draw(in: NSRect(x: p, y: p, width: w, height: h), from: .zero, operation: .sourceOver, fraction: 1)
NSGraphicsContext.restoreGraphicsState()
guard let png = out.representation(using: .png, properties: [:]) else { exit(1) }
try png.write(to: URL(fileURLWithPath: args[2]))
print("\(Int(size.width))x\(Int(size.height)) \(args[2])")
