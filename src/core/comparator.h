//
// Created by eightman on 25/07/13.
//
#ifndef COMPARATOR_H
#define COMPARATOR_H

#include <QString>
#include <QObject>
#include <vector>
#include <functional>
#include <map>
#include <set>
#include "diffresult.h"

namespace DiffLoupe {

class Comparator : public QObject {
    Q_OBJECT
    
public:
    explicit Comparator(QObject *parent = nullptr);
    ~Comparator();
    
    // 比較設定
    struct Settings {
        Settings() : showHidden(false), useMetadataFirst(true), hashBlockSize(65536), maxThreads(-1) {}
        bool showHidden;            // 隠しファイルを表示
        bool useMetadataFirst;       // メタデータで一次比較
        int hashBlockSize;          // ハッシュ計算時のブロックサイズ
        int maxThreads;                // 最大スread数（-1で自動）
    };
    
    // 進捗コールバック
    using ProgressCallback = std::function<void(const QString&, int)>;
    
    // メイン比較関数
    std::vector<DiffResult> compareAll(
        const QStringList &folders,
        const Settings &settings = Settings(),
        ProgressCallback progressCallback = nullptr
    );
    
    // 単一ファイルのハッシュ計算
    static QString hashFile(const QString &filePath, int blockSize = 65536);
    
signals:
    // 進捗通知
    void progressUpdated(const QString &message, int percentage);
    
private:
    // ディレクトリ走査
    void scanDirectory(
        const QString &basePath,
        int folderIndex,
        std::map<QString, std::vector<FileInfo>> &allFiles,
        const Settings &settings
    );
    
    // ファイルメタデータ取得
    FileInfo getFileInfo(const QString &filePath, const QString &basePath);
    
    // メタデータ比較
    bool compareMetadata(const FileInfo &a, const FileInfo &b) const;
    
    // ハッシュ計算が必要なファイルを抽出
    std::set<QString> findFilesNeedingHash(
        const std::map<QString, std::vector<FileInfo>> &allFiles,
        std::map<QString, DiffStatus> &preliminaryResults
    );
    
    // 並列ハッシュ計算
    std::map<QString, QString> computeHashesParallel(
        const std::set<QString> &filePaths,
        const Settings &settings,
        ProgressCallback progressCallback
    );
    
    // 最終結果の構築
    std::vector<DiffResult> buildResults(
        const std::map<QString, std::vector<FileInfo>> &allFiles,
        const std::map<QString, DiffStatus> &preliminaryResults,
        const std::map<QString, QString> &hashMap,
        const QStringList &folders
    );
    
    // ヘルパー関数
    bool shouldSkipFile(const QString &fileName, bool showHidden) const;
    bool shouldSkipDirectory(const QString &dirName, bool showHidden) const;
};

} // namespace DiffLoupe

#endif // COMPARATOR_H