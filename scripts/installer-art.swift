// 설치 화면 그림을 만듦 — 앱 아이콘과 같은 연한 크림·진한 노랑.
//   swift scripts/installer-art.swift
// 결과(src-tauri/installer/):
//   dmg-background.png·dmg-background@2x.png → dmg-background.tiff (macOS 디스크 이미지 창 배경, 540×380)
//   nsis-header.bmp (150×57)·nsis-sidebar.bmp (164×314) — Windows 설치 프로그램, 24비트 BMP
// 디스크 이미지 배경은 1x·2x를 한 TIFF로 묶음:
//   tiffutil -cathidpicheck src-tauri/installer/dmg-background.png src-tauri/installer/dmg-background@2x.png \
//     -out src-tauri/installer/dmg-background.tiff
import AppKit

let root = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
let outDir = root.appendingPathComponent("src-tauri/installer")
try? FileManager.default.createDirectory(at: outDir, withIntermediateDirectories: true)

let appIcon = NSImage(contentsOf: root.appendingPathComponent("src-tauri/icons/icon.png"))!
let mark = NSImage(contentsOf: root.appendingPathComponent("public/brand/mark-ggparrot.svg"))!

func rgb(_ hex: UInt32, _ a: CGFloat = 1) -> NSColor {
  NSColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
          blue: CGFloat(hex & 0xFF) / 255, alpha: a)
}
let CREAM_TOP = rgb(0xFFFEF8), CREAM_BOTTOM = rgb(0xFFF4D2)
let BRAND = rgb(0xF5C542), BRAND_DEEP = rgb(0xA16207)
let INK = rgb(0x111827), MUTED = rgb(0x6B7280)

/// 위가 원점인 좌표로 w×h 픽셀(배율 scale) 비트맵에 그림.
func render(_ w: Int, _ h: Int, scale: CGFloat = 1, _ draw: (CGRect) -> Void) -> NSBitmapImageRep {
  let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: Int(CGFloat(w) * scale), pixelsHigh: Int(CGFloat(h) * scale),
                             bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
                             colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
  rep.size = NSSize(width: w, height: h)
  let ctx = NSGraphicsContext(bitmapImageRep: rep)!
  NSGraphicsContext.saveGraphicsState()
  NSGraphicsContext.current = ctx
  ctx.imageInterpolation = .high
  let cg = ctx.cgContext
  cg.translateBy(x: 0, y: CGFloat(h))
  cg.scaleBy(x: 1, y: -1)
  NSGraphicsContext.current = NSGraphicsContext(cgContext: cg, flipped: true)
  draw(CGRect(x: 0, y: 0, width: w, height: h))
  NSGraphicsContext.restoreGraphicsState()
  return rep
}

func gradient(_ r: CGRect, _ top: NSColor, _ bottom: NSColor) {
  NSGradient(starting: top, ending: bottom)!.draw(in: r, angle: 90) // 뒤집힌 좌표: 90° = 위→아래
}

func text(_ s: String, _ size: CGFloat, _ weight: NSFont.Weight, _ color: NSColor, x: CGFloat, y: CGFloat,
          center: Bool = false, width: CGFloat = 0) {
  let p = NSMutableParagraphStyle()
  p.alignment = center ? .center : .left
  p.lineSpacing = size * 0.25
  let attrs: [NSAttributedString.Key: Any] = [.font: NSFont.systemFont(ofSize: size, weight: weight),
                                              .foregroundColor: color, .paragraphStyle: p]
  let str = NSAttributedString(string: s, attributes: attrs)
  let w = width > 0 ? width : str.size().width + 2
  str.draw(with: CGRect(x: center ? x - w / 2 : x, y: y, width: w, height: 400),
           options: [.usesLineFragmentOrigin, .usesFontLeading])
}

func drawImage(_ img: NSImage, _ r: CGRect) {
  img.draw(in: r, from: .zero, operation: .sourceOver, fraction: 1, respectFlipped: true, hints: [.interpolation: NSImageInterpolation.high])
}

// --- macOS 디스크 이미지 배경 (540×380) — 앱 (140,165)·응용 프로그램 (400,165), 아이콘 128 -----------------
func dmg(_ r: CGRect) {
  gradient(r, CREAM_TOP, CREAM_BOTTOM)
  // Finder 제목 줄·경로 막대가 창 아래 약 56pt를 가리므로 중요한 것은 위 320pt 안에 둠.

  drawImage(mark, CGRect(x: 24, y: 22, width: 30, height: 30))
  text("껄무새", 17, .bold, INK, x: 62, y: 20)
  text("GG Parrot", 11, .semibold, MUTED, x: 63, y: 41)
  text("왼쪽 앱을 오른쪽 Applications(응용 프로그램) 폴더로 끌어다 놓으면 설치돼요.", 12.5, .medium, MUTED, x: 24, y: 68)

  // 두 아이콘 사이 화살표(점선 + 머리)
  let y: CGFloat = 157
  let path = NSBezierPath()
  path.move(to: CGPoint(x: 222, y: y))
  path.line(to: CGPoint(x: 310, y: y))
  path.lineWidth = 3.5
  path.lineCapStyle = .round
  path.setLineDash([0.1, 9], count: 2, phase: 0)
  BRAND_DEEP.withAlphaComponent(0.85).setStroke()
  path.stroke()
  let head = NSBezierPath()
  head.move(to: CGPoint(x: 306, y: y - 9))
  head.line(to: CGPoint(x: 318, y: y))
  head.line(to: CGPoint(x: 306, y: y + 9))
  head.lineWidth = 3.5
  head.lineCapStyle = .round
  head.lineJoinStyle = .round
  head.stroke()

  text("설치한 뒤에는 응용 프로그램 폴더에서 실행하세요. 앱 안에서 바로 업데이트할 수 있어요.", 11, .regular,
       MUTED.withAlphaComponent(0.9), x: r.width / 2, y: 292, center: true, width: 500)
}

// --- Windows 설치 프로그램: 머리 그림(150×57, 흰 머리줄 왼쪽) ------------------------------------------
func nsisHeader(_ r: CGRect) {
  NSColor.white.setFill()
  NSBezierPath(rect: r).fill()
  drawImage(appIcon, CGRect(x: 10, y: 6, width: 45, height: 45))
  text("껄무새", 15, .heavy, INK, x: 62, y: 10)
  text("GG Parrot", 10.5, .semibold, MUTED, x: 62, y: 31)
}

// --- Windows 설치 프로그램: 시작·마침 화면 왼쪽 그림(164×314) ---------------------------------------------
func nsisSidebar(_ r: CGRect) {
  gradient(r, CREAM_TOP, CREAM_BOTTOM)
  BRAND.setFill()
  NSBezierPath(rect: CGRect(x: 0, y: 0, width: r.width, height: 5)).fill()
  drawImage(appIcon, CGRect(x: (r.width - 104) / 2, y: 46, width: 104, height: 104))
  text("껄무새", 22, .heavy, INK, x: r.width / 2, y: 164, center: true, width: r.width)
  text("GG Parrot", 13, .bold, MUTED, x: r.width / 2, y: 196, center: true, width: r.width)
  BRAND.setFill()
  NSBezierPath(roundedRect: CGRect(x: r.width / 2 - 14, y: 224, width: 28, height: 3), xRadius: 1.5, yRadius: 1.5).fill()
  text("웹에서 만든 매크로를\n이 PC에서 실행해요", 10.5, .medium, MUTED, x: r.width / 2, y: 240, center: true, width: r.width)
}

// --- 저장 -------------------------------------------------------------------------------------------
func savePNG(_ rep: NSBitmapImageRep, _ name: String) {
  try! rep.representation(using: .png, properties: [:])!.write(to: outDir.appendingPathComponent(name))
}

/// 24비트 BMP(아래에서 위로, 줄마다 4바이트 맞춤) — NSIS가 가장 확실히 읽는 형식.
func saveBMP24(_ rep: NSBitmapImageRep, _ name: String) {
  let w = rep.pixelsWide, h = rep.pixelsHigh
  let rowSize = (w * 3 + 3) & ~3
  let imageSize = rowSize * h
  var d = Data()
  func u16(_ v: Int) { var x = UInt16(v).littleEndian; d.append(Data(bytes: &x, count: 2)) }
  func u32(_ v: Int) { var x = UInt32(v).littleEndian; d.append(Data(bytes: &x, count: 4)) }
  d.append(contentsOf: [0x42, 0x4D]); u32(54 + imageSize); u32(0); u32(54)
  u32(40); u32(w); u32(h); u16(1); u16(24); u32(0); u32(imageSize); u32(2835); u32(2835); u32(0); u32(0)
  for y in stride(from: h - 1, through: 0, by: -1) {
    var row = [UInt8](repeating: 0, count: rowSize)
    for x in 0..<w {
      let c = rep.colorAt(x: x, y: y)!.usingColorSpace(.sRGB)!
      // 투명한 곳은 흰 바탕과 섞음
      let a = c.alphaComponent
      func ch(_ v: CGFloat) -> UInt8 { UInt8(max(0, min(255, (v * a + (1 - a)) * 255 + 0.5))) }
      row[x * 3] = ch(c.blueComponent); row[x * 3 + 1] = ch(c.greenComponent); row[x * 3 + 2] = ch(c.redComponent)
    }
    d.append(contentsOf: row)
  }
  try! d.write(to: outDir.appendingPathComponent(name))
}

savePNG(render(540, 380, scale: 1, dmg), "dmg-background.png")
savePNG(render(540, 380, scale: 2, dmg), "dmg-background@2x.png")
saveBMP24(render(150, 57, nsisHeader), "nsis-header.bmp")
saveBMP24(render(164, 314, nsisSidebar), "nsis-sidebar.bmp")
print("ok")
