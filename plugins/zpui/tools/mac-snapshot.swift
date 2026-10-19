import AppKit
import WebKit

// Local acceptance helper only; not part of the adapter's runtime.
@_cdecl("crepus_take_snapshot")
public func takeSnapshot(_ raw: UnsafeMutableRawPointer, _ path: UnsafePointer<CChar>,
                         _ context: UnsafeMutableRawPointer?,
                         _ done: @convention(c) (UnsafeMutableRawPointer?, Bool) -> Void) {
    let web = Unmanaged<WKWebView>.fromOpaque(raw).takeUnretainedValue()
    let destination = String(cString: path)
    web.takeSnapshot(with: nil) { image, error in
        guard error == nil, let data = image?.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: data), bitmap.pixelsWide > 40,
              bitmap.pixelsHigh > 40,
              let color = bitmap.colorAt(x: 20, y: 20)?.usingColorSpace(.sRGB),
              let png = bitmap.representation(using: .png, properties: [:]) else {
            done(context, false); return
        }
        // Verify exported PNG bytes, avoiding a second color-space conversion of
        // the TIFF representation's calibrated NSColor sample.
        guard let exported = NSBitmapImageRep(data: png), exported.bitsPerSample == 8,
              exported.samplesPerPixel == 4, let pixels = exported.bitmapData else {
            done(context, false); return
        }
        let offset = 20 * exported.bytesPerRow + 20 * exported.samplesPerPixel
        let first = exported.bitmapFormat.contains(.alphaFirst) ? 1 : 0
        let correct = abs(Int(pixels[offset + first]) - 18) <= 2 &&
                      abs(Int(pixels[offset + first + 1]) - 52) <= 2 &&
                      abs(Int(pixels[offset + first + 2]) - 86) <= 2
        _ = color
        do { try png.write(to: URL(fileURLWithPath: destination)); done(context, correct) }
        catch { done(context, false) }
    }
}
