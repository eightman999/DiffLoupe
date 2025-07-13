//
// Created by eightman on 25/07/13.
//
#include "core/comparator.h"
#include <QDirIterator>
#include <QDir>
#include <QFile>
#include <QCryptographicHash>
#include <QFileInfo>
#include <QThread>
#include <QtConcurrent>
#include <map>
#include <set>

namespace DiffLoupe {

Comparator::Comparator(QObject *parent) : QObject(parent)
{
}

Comparator::~Comparator() = default;

std::vector<DiffResult> Comparator::compareAll(
    const QStringList &folders,
    const Settings &settings,
    ProgressCallback progressCallback)
{
    if (folders.size() < 2) {
        return {};
    }

    emit progressUpdated("ファイルリストを収集中...", 0);

    // 1. すべてのフォルダからファイルリストを収集
    std::map<QString, std::vector<FileInfo>> allFiles;

    for (int i = 0; i < folders.size(); ++i) {
        scanDirectory(folders[i], i, allFiles, settings);
    }

    // 2. メタデータで一次比較
    std::map<QString, DiffStatus> preliminaryResults;
    std::set<QString> needsHash;

    if (settings.useMetadataFirst) {
        needsHash = findFilesNeedingHash(allFiles, preliminaryResults);
    } else {
        // すべてのファイルのハッシュが必要
        for (const auto& [relPath, files] : allFiles) {
            for (const auto& file : files) {
                if (file.exists()) {
                    needsHash.insert(file.filePath);
                }
            }
        }
    }

    // 3. 必要なファイルのハッシュを並列計算
    std::map<QString, QString> hashMap;
    if (!needsHash.empty()) {
        emit progressUpdated("ファイルハッシュを計算中...", 30);
        hashMap = computeHashesParallel(needsHash, settings, progressCallback);
    }

    // 4. 最終結果を構築
    emit progressUpdated("結果を整理中...", 90);
    auto results = buildResults(allFiles, preliminaryResults, hashMap, folders);

    emit progressUpdated("完了", 100);
    return results;
}

void Comparator::scanDirectory(
    const QString &basePath,
    int folderIndex,
    std::map<QString, std::vector<FileInfo>> &allFiles,
    const Settings &settings)
{
    QDirIterator it(basePath, QDir::Files | QDir::Dirs | QDir::NoDotAndDotDot,
                    QDirIterator::Subdirectories);

    while (it.hasNext()) {
        it.next();

        if (it.fileInfo().isDir()) {
            if (shouldSkipDirectory(it.fileName(), settings.showHidden)) {
                it.next(); // スキップ
                continue;
            }
        } else {
            if (shouldSkipFile(it.fileName(), settings.showHidden)) {
                continue;
            }

            QString relativePath = QDir(basePath).relativeFilePath(it.filePath());
            FileInfo info = getFileInfo(it.filePath(), basePath);

            // allFilesに追加（必要に応じてvectorを拡張）
            auto& fileVector = allFiles[relativePath];
            if (fileVector.size() <= folderIndex) {
                fileVector.resize(folderIndex + 1);
            }
            fileVector[folderIndex] = info;
        }
    }
}

FileInfo Comparator::getFileInfo(const QString &filePath, const QString &basePath)
{
    FileInfo info;
    info.folderPath = basePath;
    info.filePath = filePath;

    QFileInfo fi(filePath);
    if (fi.exists()) {
        info.size = fi.size();
        info.mtime = fi.lastModified();
    }

    return info;
}

bool Comparator::compareMetadata(const FileInfo &a, const FileInfo &b) const
{
    return a.size == b.size && a.mtime == b.mtime;
}

std::set<QString> Comparator::findFilesNeedingHash(
    const std::map<QString, std::vector<FileInfo>> &allFiles,
    std::map<QString, DiffStatus> &preliminaryResults)
{
    std::set<QString> needsHash;

    for (const auto& [relPath, files] : allFiles) {
        if (files.size() < 2) {
            // 片方にしか存在しない
            preliminaryResults[relPath] = files[0].exists() ?
                DiffStatus::MissingInB : DiffStatus::MissingInA;
            continue;
        }

        // 両方に存在する場合
        const auto& fileA = files[0];
        const auto& fileB = files[1];

        if (!fileA.exists()) {
            preliminaryResults[relPath] = DiffStatus::MissingInA;
        } else if (!fileB.exists()) {
            preliminaryResults[relPath] = DiffStatus::MissingInB;
        } else if (compareMetadata(fileA, fileB)) {
            preliminaryResults[relPath] = DiffStatus::MatchMetadata;
        } else {
            // メタデータが異なる場合はハッシュ計算が必要
            needsHash.insert(fileA.filePath);
            needsHash.insert(fileB.filePath);
        }
    }

    return needsHash;
}

QString Comparator::hashFile(const QString &filePath, int blockSize)
{
    QFile file(filePath);
    if (!file.open(QIODevice::ReadOnly)) {
        return QString();
    }

    QCryptographicHash hash(QCryptographicHash::Sha256);

    while (!file.atEnd()) {
        QByteArray chunk = file.read(blockSize);
        hash.addData(chunk);
    }

    return hash.result().toHex();
}

std::map<QString, QString> Comparator::computeHashesParallel(
    const std::set<QString> &filePaths,
    const Settings &settings,
    ProgressCallback progressCallback)
{
    QList<QString> pathList(filePaths.begin(), filePaths.end());
    std::map<QString, QString> hashMap;

    // シンプルな並列処理実装
    int maxThreads = settings.maxThreads > 0 ? settings.maxThreads : QThread::idealThreadCount();

    // QtConcurrentを使用しない場合の実装
    // TODO: QtConcurrent::mappedを使用した並列化
    for (const QString &path : pathList) {
        hashMap[path] = hashFile(path, settings.hashBlockSize);
    }

    return hashMap;
}

std::vector<DiffResult> Comparator::buildResults(
    const std::map<QString, std::vector<FileInfo>> &allFiles,
    const std::map<QString, DiffStatus> &preliminaryResults,
    const std::map<QString, QString> &hashMap,
    const QStringList &folders)
{
    std::vector<DiffResult> results;

    for (const auto& [relPath, files] : allFiles) {
        DiffResult result;
        result.relativePath = relPath;
        result.files = files;

        // ハッシュ値を設定
        for (auto& file : result.files) {
            if (file.exists() && hashMap.count(file.filePath)) {
                file.hash = hashMap.at(file.filePath);
            }
        }

        // ステータスを判定
        if (preliminaryResults.count(relPath)) {
            result.status = preliminaryResults.at(relPath);
        } else {
            // ハッシュで比較
            std::set<QString> uniqueHashes;
            for (const auto& file : result.files) {
                if (!file.hash.isEmpty()) {
                    uniqueHashes.insert(file.hash);
                }
            }

            result.status = (uniqueHashes.size() <= 1) ?
                DiffStatus::Match : DiffStatus::Different;
        }

        results.push_back(result);
    }

    return results;
}

bool Comparator::shouldSkipFile(const QString &fileName, bool showHidden) const
{
    return !showHidden && fileName.startsWith('.');
}

bool Comparator::shouldSkipDirectory(const QString &dirName, bool showHidden) const
{
    return !showHidden && dirName.startsWith('.');
}

} // namespace DiffLoupe