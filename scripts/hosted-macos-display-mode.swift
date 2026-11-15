import CoreGraphics
import Foundation

let minWidth = Int(CommandLine.arguments.dropFirst().first ?? "1280") ?? 1280
let minHeight = Int(CommandLine.arguments.dropFirst(2).first ?? "900") ?? 900
let display = CGMainDisplayID()
let options = [kCGDisplayShowDuplicateLowResolutionModes: kCFBooleanTrue] as CFDictionary
let modes = (CGDisplayCopyAllDisplayModes(display, options) as? [CGDisplayMode]) ?? []
print("current: \(CGDisplayPixelsWide(display))x\(CGDisplayPixelsHigh(display))")
for mode in modes {
    print("mode: \(mode.width)x\(mode.height) pixels \(mode.pixelWidth)x\(mode.pixelHeight) usable=\(mode.isUsableForDesktopGUI())")
}
let fitting = modes
    .filter { $0.width >= minWidth && $0.height >= minHeight && $0.isUsableForDesktopGUI() }
    .sorted { $0.width * $0.height < $1.width * $1.height }
guard let chosen = fitting.first else {
    print("no mode of at least \(minWidth)x\(minHeight)")
    exit(1)
}
var config: CGDisplayConfigRef?
guard CGBeginDisplayConfiguration(&config) == .success, let config else {
    print("CGBeginDisplayConfiguration failed")
    exit(1)
}
let set = CGConfigureDisplayWithDisplayMode(config, display, chosen, nil)
let done = CGCompleteDisplayConfiguration(config, .permanently)
print("set \(chosen.width)x\(chosen.height): configure=\(set.rawValue) complete=\(done.rawValue)")
print("now: \(CGDisplayPixelsWide(display))x\(CGDisplayPixelsHigh(display))")
exit(set == .success && done == .success ? 0 : 1)
