// Copyright (c) eightman 2005-2026
// Furin-lab All rights reserved.
// 動作設計: GitRepository(detect/listRefs/materializeSnapshot)を実gitフィクスチャで検証する

import XCTest
@testable import DiffLoupe

final class GitSnapshotTests: XCTestCase {

    private static let gitExecutablePath = "/usr/bin/git"

    private var fixtureRepoURL: URL?

    override func tearDownWithError() throws {
        if let fixtureRepoURL {
            try? FileManager.default.removeItem(at: fixtureRepoURL)
        }
        fixtureRepoURL = nil
        try super.tearDownWithError()
    }

    /// gitバイナリが使えない環境ではスキップする
    private func skipIfGitUnavailable() throws {
        guard FileManager.default.isExecutableFile(atPath: Self.gitExecutablePath) else {
            throw XCTSkip("/usr/bin/git が利用できないためスキップします")
        }
    }

    /// 一時ディレクトリに実git(init → user設定 → ファイル作成 → commit → ブランチ作成)のフィクスチャを構築する
    @discardableResult
    private func makeFixtureRepo() throws -> URL {
        let repoURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupeTests-git-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: repoURL, withIntermediateDirectories: true)
        fixtureRepoURL = repoURL

        try runGit(["init", "-b", "main"], in: repoURL)
        try runGit(["config", "user.email", "test@diffloupe.example"], in: repoURL)
        try runGit(["config", "user.name", "DiffLoupe Test"], in: repoURL)

        let fileURL = repoURL.appendingPathComponent("hello.txt")
        try "hello from main\n".write(to: fileURL, atomically: true, encoding: .utf8)
        try runGit(["add", "."], in: repoURL)
        try runGit(["commit", "-m", "initial commit"], in: repoURL)

        try runGit(["branch", "feature"], in: repoURL)
        try runGit(["checkout", "feature"], in: repoURL)
        try "hello from feature\n".write(to: fileURL, atomically: true, encoding: .utf8)
        try runGit(["commit", "-am", "feature commit"], in: repoURL)
        try runGit(["checkout", "main"], in: repoURL)

        return repoURL
    }

    @discardableResult
    private func runGit(_ arguments: [String], in directory: URL) throws -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: Self.gitExecutablePath)
        process.arguments = arguments
        process.currentDirectoryURL = directory

        let outPipe = Pipe()
        let errPipe = Pipe()
        process.standardOutput = outPipe
        process.standardError = errPipe

        try process.run()
        let outData = outPipe.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()

        guard process.terminationStatus == 0 else {
            let errData = errPipe.fileHandleForReading.readDataToEndOfFile()
            let message = String(data: errData, encoding: .utf8) ?? "unknown git error"
            throw NSError(domain: "GitSnapshotTests", code: Int(process.terminationStatus), userInfo: [
                NSLocalizedDescriptionKey: "git \(arguments.joined(separator: " ")) 失敗: \(message)"
            ])
        }
        return String(data: outData, encoding: .utf8) ?? ""
    }

    // MARK: - detect

    func testDetectFindsRepositoryAtToplevel() throws {
        try skipIfGitUnavailable()
        let repoURL = try makeFixtureRepo()

        let detected = GitRepository.detect(url: repoURL)
        XCTAssertNotNil(detected)
        XCTAssertEqual(
            detected?.toplevel.resolvingSymlinksInPath().path,
            repoURL.resolvingSymlinksInPath().path
        )
    }

    func testDetectFindsRepositoryFromSubdirectory() throws {
        try skipIfGitUnavailable()
        let repoURL = try makeFixtureRepo()
        let subdir = repoURL.appendingPathComponent("sub")
        try FileManager.default.createDirectory(at: subdir, withIntermediateDirectories: true)

        let detected = GitRepository.detect(url: subdir)
        XCTAssertNotNil(detected)
    }

    func testDetectReturnsNilForNonRepository() throws {
        try skipIfGitUnavailable()
        let plainDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("DiffLoupeTests-plain-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: plainDir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: plainDir) }

        XCTAssertNil(GitRepository.detect(url: plainDir))
    }

    // MARK: - listRefs

    func testListRefsIncludesHeadAndBranches() throws {
        try skipIfGitUnavailable()
        let repoURL = try makeFixtureRepo()
        guard let repo = GitRepository.detect(url: repoURL) else {
            XCTFail("リポジトリ検出に失敗")
            return
        }

        let refs = repo.listRefs()
        XCTAssertEqual(refs.first, "HEAD")
        XCTAssertTrue(refs.contains("main"))
        XCTAssertTrue(refs.contains("feature"))
    }

    // MARK: - materializeSnapshot

    func testMaterializeSnapshotExtractsBranchContent() throws {
        try skipIfGitUnavailable()
        let repoURL = try makeFixtureRepo()
        guard let repo = GitRepository.detect(url: repoURL) else {
            XCTFail("リポジトリ検出に失敗")
            return
        }

        let snapshot = try repo.materializeSnapshot(ref: "feature")
        defer { GitRepository.cleanup(snapshot) }

        let content = try String(
            contentsOf: snapshot.appendingPathComponent("hello.txt"),
            encoding: .utf8
        )
        XCTAssertEqual(content, "hello from feature\n")
    }

    func testMaterializeSnapshotDiffersFromMain() throws {
        try skipIfGitUnavailable()
        let repoURL = try makeFixtureRepo()
        guard let repo = GitRepository.detect(url: repoURL) else {
            XCTFail("リポジトリ検出に失敗")
            return
        }

        let mainSnapshot = try repo.materializeSnapshot(ref: "main")
        defer { GitRepository.cleanup(mainSnapshot) }
        let mainContent = try String(
            contentsOf: mainSnapshot.appendingPathComponent("hello.txt"),
            encoding: .utf8
        )
        XCTAssertEqual(mainContent, "hello from main\n")
    }

    func testMaterializeSnapshotThrowsForUnknownRef() throws {
        try skipIfGitUnavailable()
        let repoURL = try makeFixtureRepo()
        guard let repo = GitRepository.detect(url: repoURL) else {
            XCTFail("リポジトリ検出に失敗")
            return
        }

        XCTAssertThrowsError(try repo.materializeSnapshot(ref: "does-not-exist-ref")) { error in
            XCTAssertTrue(error is GitRepository.GitError)
        }
    }

    func testCleanupRemovesSnapshotDirectory() throws {
        try skipIfGitUnavailable()
        let repoURL = try makeFixtureRepo()
        guard let repo = GitRepository.detect(url: repoURL) else {
            XCTFail("リポジトリ検出に失敗")
            return
        }

        let snapshot = try repo.materializeSnapshot(ref: "main")
        XCTAssertTrue(FileManager.default.fileExists(atPath: snapshot.path))

        GitRepository.cleanup(snapshot)
        XCTAssertFalse(FileManager.default.fileExists(atPath: snapshot.path))
    }
}
