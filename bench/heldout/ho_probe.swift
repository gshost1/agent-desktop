// Held-out verifier helper. Reads bench files and system names only; never opens a window.
//   ho_probe rtf <file.rtf>              paragraphs with alignment and attribute runs
//   ho_probe pixels <image> <cols> <rows> image size and the colour at each grid cell centre
//   ho_probe tz <IANA zone>              the names macOS shows for that zone
import AppKit
import Foundation

func emit(_ value: Any) {
    let data = try! JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write("\n".data(using: .utf8)!)
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write((message + "\n").data(using: .utf8)!)
    exit(1)
}

func alignmentName(_ a: NSTextAlignment) -> String {
    switch a {
    case .center: return "center"
    case .right: return "right"
    case .justified: return "justified"
    case .left: return "left"
    default: return "natural"
    }
}

func rtf(_ path: String) {
    let url = URL(fileURLWithPath: path)
    guard let text = try? NSAttributedString(url: url, options: [:], documentAttributes: nil) else {
        fail("cannot read \(path)")
    }
    let whole = text.string as NSString
    var paragraphs: [[String: Any]] = []
    var start = 0
    while start <= whole.length {
        var end = 0, contentsEnd = 0
        whole.getParagraphStart(nil, end: &end, contentsEnd: &contentsEnd, for: NSRange(location: start, length: 0))
        let range = NSRange(location: start, length: contentsEnd - start)
        var runs: [[String: Any]] = []
        var alignment = "natural"
        if range.length > 0 {
            if let style = text.attribute(.paragraphStyle, at: range.location, effectiveRange: nil) as? NSParagraphStyle {
                alignment = alignmentName(style.alignment)
            }
            text.enumerateAttributes(in: range, options: []) { attrs, sub, _ in
                let font = attrs[.font] as? NSFont
                let traits = font.map { NSFontManager.shared.traits(of: $0) } ?? []
                let strike = (attrs[.strikethroughStyle] as? Int ?? 0) != 0
                let script = attrs[NSAttributedString.Key("NSSuperScript")] as? Int ?? 0
                let baseline = attrs[.baselineOffset] as? Double ?? 0
                runs.append(["text": whole.substring(with: sub), "bold": traits.contains(.boldFontMask),
                             "italic": traits.contains(.italicFontMask), "strike": strike, "script": script,
                             "baseline": baseline, "size": Double(font?.pointSize ?? 0), "font": font?.fontName ?? ""])
            }
        }
        paragraphs.append(["text": whole.substring(with: range), "alignment": alignment, "runs": runs])
        if end <= start { break }
        start = end
        if start == whole.length { break }
    }
    emit(["paragraphs": paragraphs])
}

func pixels(_ path: String, _ cols: Int, _ rows: Int) {
    guard let data = FileManager.default.contents(atPath: path), let rep = NSBitmapImageRep(data: data) else {
        fail("cannot read image \(path)")
    }
    let w = rep.pixelsWide, h = rep.pixelsHigh
    var grid: [[[Int]]] = []
    for j in 0..<rows {
        var row: [[Int]] = []
        for i in 0..<cols {
            let x = Int((Double(i) + 0.5) / Double(cols) * Double(w))
            let y = Int((Double(j) + 0.5) / Double(rows) * Double(h))
            let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.sRGB)
            row.append([Int((c?.redComponent ?? 0) * 255), Int((c?.greenComponent ?? 0) * 255), Int((c?.blueComponent ?? 0) * 255)])
        }
        grid.append(row)
    }
    emit(["width": w, "height": h, "grid": grid])
}

func tz(_ id: String) {
    guard let zone = TimeZone(identifier: id) else { fail("unknown zone \(id)") }
    let locale = Locale(identifier: "en_US")
    let styles: [NSTimeZone.NameStyle] = [.standard, .generic, .daylightSaving, .shortStandard, .shortGeneric]
    let names = styles.compactMap { zone.localizedName(for: $0, locale: locale) }
    emit(["names": Array(Set(names)).sorted(), "abbreviation": zone.abbreviation() ?? ""])
}

let args = CommandLine.arguments.dropFirst()
switch args.first {
case "rtf" where args.count == 2: rtf(args[args.startIndex + 1])
case "pixels" where args.count == 4:
    pixels(args[args.startIndex + 1], Int(args[args.startIndex + 2]) ?? 12, Int(args[args.startIndex + 3]) ?? 8)
case "tz" where args.count == 2: tz(args[args.startIndex + 1])
default: fail("usage: ho_probe rtf <file> | pixels <image> <cols> <rows> | tz <zone>")
}
