// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: 画像比較エンジンの形式検出・メタ情報・ピクセル差分を検証する

import CoreGraphics
import ImageIO
import XCTest
@testable import DiffLoupe

final class ImageDiffEngineTests: XCTestCase {

    func testSupportedImageExtensions() {
        XCTAssertTrue(ImageFileDetector.isSupportedImageFile(URL(fileURLWithPath: "/tmp/photo.JPG")))
        XCTAssertTrue(ImageFileDetector.isSupportedImageFile(URL(fileURLWithPath: "/tmp/photo.heic")))
        XCTAssertTrue(ImageFileDetector.isSupportedImageFile(URL(fileURLWithPath: "/tmp/photo.tiff")))
        XCTAssertFalse(ImageFileDetector.isSupportedImageFile(URL(fileURLWithPath: "/tmp/photo.txt")))
    }

    func testComparePNGReportsMetadataAndPixelDiff() throws {
        let leftURL = try writePNG(name: "left.png", width: 2, height: 1, rgba: [
            255, 0, 0, 255,
            0, 255, 0, 255
        ])
        let rightURL = try writePNG(name: "right.png", width: 2, height: 1, rgba: [
            255, 0, 0, 255,
            0, 0, 255, 255
        ])

        let result = try ImageDiffEngine.compare(leftURL: leftURL, rightURL: rightURL, threshold: 30)

        XCTAssertEqual(result.leftMetadata.width, 2)
        XCTAssertEqual(result.leftMetadata.height, 1)
        XCTAssertEqual(result.leftMetadata.format, "PNG")
        XCTAssertEqual(result.rightMetadata.width, 2)
        XCTAssertEqual(result.rightMetadata.height, 1)
        XCTAssertEqual(result.stats.comparedWidth, 2)
        XCTAssertEqual(result.stats.comparedHeight, 1)
        XCTAssertEqual(result.stats.diffPixelCount, 1)
        XCTAssertEqual(result.stats.diffPercentage, 50, accuracy: 0.001)
        XCTAssertFalse(result.hasDifferentDimensions)
    }

    private func writePNG(name: String, width: Int, height: Int, rgba: [UInt8]) throws -> URL {
        XCTAssertEqual(rgba.count, width * height * 4)
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("ImageDiffEngineTests-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        addTeardownBlock { try? FileManager.default.removeItem(at: directory) }

        let url = directory.appendingPathComponent(name)
        let data = Data(rgba)
        guard let provider = CGDataProvider(data: data as CFData),
              let image = CGImage(
                width: width,
                height: height,
                bitsPerComponent: 8,
                bitsPerPixel: 32,
                bytesPerRow: width * 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue),
                provider: provider,
                decode: nil,
                shouldInterpolate: false,
                intent: .defaultIntent
              ),
              let destination = CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil) else {
            XCTFail("PNGテスト画像の生成に失敗しました")
            return url
        }

        CGImageDestinationAddImage(destination, image, nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination))
        return url
    }
}
