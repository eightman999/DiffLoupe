// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 画像ファイル検出・メタ情報取得・ピクセル差分画像生成を標準フレームワークだけで行う

import AppKit
import CoreImage
import Foundation
import ImageIO

enum ImageFileDetector {
    static let supportedExtensions: Set<String> = [
        "png", "jpg", "jpeg", "jpe", "gif", "bmp",
        "tif", "tiff", "heic", "heif", "webp"
    ]

    static func isSupportedImageFile(_ url: URL) -> Bool {
        supportedExtensions.contains(url.pathExtension.lowercased())
    }
}

struct ImageFileMetadata: Sendable, Equatable {
    let width: Int
    let height: Int
    let fileSize: Int64
    let format: String

    var dimensionsText: String {
        guard width > 0, height > 0 else { return "不明" }
        return "\(width) x \(height) px"
    }

    var fileSizeText: String {
        ByteCountFormatter.string(fromByteCount: fileSize, countStyle: .file)
    }
}

struct ImageDiffStats: Sendable, Equatable {
    let comparedWidth: Int
    let comparedHeight: Int
    let diffPixelCount: Int
    let diffPercentage: Double
    let threshold: Int
    let usedDownsampledComparison: Bool

    var comparedDimensionsText: String {
        "\(comparedWidth) x \(comparedHeight) px"
    }
}

struct ImageComparisonResult {
    let leftImage: CGImage
    let rightImage: CGImage
    let diffImage: CGImage
    let leftMetadata: ImageFileMetadata
    let rightMetadata: ImageFileMetadata
    let stats: ImageDiffStats

    var hasDifferentDimensions: Bool {
        leftMetadata.width != rightMetadata.width || leftMetadata.height != rightMetadata.height
    }
}

enum ImageDiffEngine {
    static let defaultThreshold = 30
    static let maxComparisonPixels = 16_000_000

    enum ImageError: LocalizedError {
        case unreadable(URL)
        case unsupportedBitmap

        var errorDescription: String? {
            switch self {
            case .unreadable(let url):
                return "画像を読み込めません: \(url.lastPathComponent)"
            case .unsupportedBitmap:
                return "画像のビットマップ変換に失敗しました"
            }
        }
    }

    static func compare(leftURL: URL, rightURL: URL, threshold: Int = defaultThreshold) throws -> ImageComparisonResult {
        let left = try loadImage(url: leftURL)
        let right = try loadImage(url: rightURL)
        let canvas = comparisonCanvas(left: left.image, right: right.image)

        let leftPixels = try rgbaPixels(from: left.image, width: canvas.width, height: canvas.height)
        let rightPixels = try rgbaPixels(from: right.image, width: canvas.width, height: canvas.height)
        let generated = try generateDiffImage(
            leftPixels: leftPixels,
            rightPixels: rightPixels,
            width: canvas.width,
            height: canvas.height,
            threshold: threshold
        )

        let totalPixels = max(canvas.width * canvas.height, 1)
        let percentage = Double(generated.diffPixelCount) / Double(totalPixels) * 100
        let stats = ImageDiffStats(
            comparedWidth: canvas.width,
            comparedHeight: canvas.height,
            diffPixelCount: generated.diffPixelCount,
            diffPercentage: percentage,
            threshold: threshold,
            usedDownsampledComparison: canvas.downsampled
        )

        return ImageComparisonResult(
            leftImage: left.image,
            rightImage: right.image,
            diffImage: generated.image,
            leftMetadata: left.metadata,
            rightMetadata: right.metadata,
            stats: stats
        )
    }

    private struct LoadedImage {
        let image: CGImage
        let metadata: ImageFileMetadata
    }

    private struct ComparisonCanvas {
        let width: Int
        let height: Int
        let downsampled: Bool
    }

    private static func loadImage(url: URL) throws -> LoadedImage {
        let image = try loadOrientedCGImage(url: url)
        let fileSize = Int64((try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0)
        let metadata = ImageFileMetadata(
            width: image.width,
            height: image.height,
            fileSize: fileSize,
            format: url.pathExtension.isEmpty ? "不明" : url.pathExtension.uppercased()
        )
        return LoadedImage(image: image, metadata: metadata)
    }

    private static func loadOrientedCGImage(url: URL) throws -> CGImage {
        let context = CIContext(options: [.cacheIntermediates: false])
        if let ciImage = CIImage(contentsOf: url, options: [.applyOrientationProperty: true]) {
            let rect = ciImage.extent.integral
            if let image = context.createCGImage(ciImage, from: rect) {
                return image
            }
        }

        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
              let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
            throw ImageError.unreadable(url)
        }
        return image
    }

    private static func comparisonCanvas(left: CGImage, right: CGImage) -> ComparisonCanvas {
        var width = max(left.width, right.width)
        var height = max(left.height, right.height)
        let pixels = width * height
        guard pixels > maxComparisonPixels else {
            return ComparisonCanvas(width: width, height: height, downsampled: false)
        }

        let scale = sqrt(Double(maxComparisonPixels) / Double(pixels))
        width = max(1, Int(Double(width) * scale))
        height = max(1, Int(Double(height) * scale))
        return ComparisonCanvas(width: width, height: height, downsampled: true)
    }

    private static func rgbaPixels(from image: CGImage, width: Int, height: Int) throws -> [UInt8] {
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        let colorSpace = CGColorSpaceCreateDeviceRGB()
        let bitmapInfo = CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue

        guard let context = CGContext(
            data: &pixels,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: colorSpace,
            bitmapInfo: bitmapInfo
        ) else {
            throw ImageError.unsupportedBitmap
        }

        context.interpolationQuality = .high
        context.clear(CGRect(x: 0, y: 0, width: width, height: height))
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        return pixels
    }

    private static func generateDiffImage(
        leftPixels: [UInt8],
        rightPixels: [UInt8],
        width: Int,
        height: Int,
        threshold: Int
    ) throws -> (image: CGImage, diffPixelCount: Int) {
        var output = [UInt8](repeating: 255, count: width * height * 4)
        var diffPixelCount = 0

        for index in stride(from: 0, to: leftPixels.count, by: 4) {
            let base = compositeOnWhite(
                r: leftPixels[index],
                g: leftPixels[index + 1],
                b: leftPixels[index + 2],
                a: leftPixels[index + 3]
            )

            let distance =
                abs(Int(leftPixels[index]) - Int(rightPixels[index])) +
                abs(Int(leftPixels[index + 1]) - Int(rightPixels[index + 1])) +
                abs(Int(leftPixels[index + 2]) - Int(rightPixels[index + 2])) +
                abs(Int(leftPixels[index + 3]) - Int(rightPixels[index + 3]))

            if distance > threshold {
                output[index] = blend(base.r, overlay: 255, alpha: 0.68)
                output[index + 1] = blend(base.g, overlay: 0, alpha: 0.68)
                output[index + 2] = blend(base.b, overlay: 0, alpha: 0.68)
                output[index + 3] = 255
                diffPixelCount += 1
            } else {
                output[index] = base.r
                output[index + 1] = base.g
                output[index + 2] = base.b
                output[index + 3] = 255
            }
        }

        guard let image = makeImage(from: output, width: width, height: height) else {
            throw ImageError.unsupportedBitmap
        }
        return (image, diffPixelCount)
    }

    private static func compositeOnWhite(r: UInt8, g: UInt8, b: UInt8, a: UInt8) -> (r: UInt8, g: UInt8, b: UInt8) {
        let alpha = Double(a) / 255
        return (
            UInt8((Double(r) * alpha + 255 * (1 - alpha)).rounded()),
            UInt8((Double(g) * alpha + 255 * (1 - alpha)).rounded()),
            UInt8((Double(b) * alpha + 255 * (1 - alpha)).rounded())
        )
    }

    private static func blend(_ base: UInt8, overlay: UInt8, alpha: Double) -> UInt8 {
        UInt8((Double(base) * (1 - alpha) + Double(overlay) * alpha).rounded())
    }

    private static func makeImage(from pixels: [UInt8], width: Int, height: Int) -> CGImage? {
        let data = Data(pixels)
        guard let provider = CGDataProvider(data: data as CFData) else { return nil }
        return CGImage(
            width: width,
            height: height,
            bitsPerComponent: 8,
            bitsPerPixel: 32,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue),
            provider: provider,
            decode: nil,
            shouldInterpolate: true,
            intent: .defaultIntent
        )
    }
}
