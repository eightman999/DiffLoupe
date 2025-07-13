//
// Created by eightman on 25/07/13.
//

#ifndef DIFFRESULT_H
#define DIFFRESULT_H

#include <QString>
#include <QDateTime>
#include <QColor>
#include <vector>

namespace DiffLoupe {

// ファイル情報
struct FileInfo {
    QString folderPath;     // 所属フォルダパス
    QString filePath;       // フルパス（nullptrの場合は欠落）
    QString hash;           // ファイルハッシュ（SHA256）
    qint64 size = 0;        // ファイルサイズ
    QDateTime mtime;        // 更新日時
    
    bool exists() const { return !filePath.isEmpty(); }
};

// 差分ステータス
enum class DiffStatus {
    Match,              // 一致
    MatchMetadata,      // メタデータ一致（ハッシュ未計算）
    Different,          // 差異
    MissingInA,         // Aに欠落
    MissingInB,         // Bに欠落
};

// 差分結果
struct DiffResult {
    QString relativePath;           // 相対パス
    DiffStatus status;              // ステータス
    std::vector<FileInfo> files;    // 各フォルダのファイル情報
    
    // ヘルパー関数
    bool isMatch() const { 
        return status == DiffStatus::Match || status == DiffStatus::MatchMetadata; 
    }
    
    bool isDifferent() const { 
        return status == DiffStatus::Different; 
    }
    
    bool isMissing() const { 
        return status == DiffStatus::MissingInA || status == DiffStatus::MissingInB; 
    }
    
    bool existsInBoth() const {
        return files.size() >= 2 && files[0].exists() && files[1].exists();
    }
    
    qint64 getMaxSize() const {
        qint64 maxSize = 0;
        for (const auto& file : files) {
            if (file.size > maxSize) {
                maxSize = file.size;
            }
        }
        return maxSize;
    }
    
    QDateTime getLatestMTime() const {
        QDateTime latest;
        for (const auto& file : files) {
            if (file.mtime.isValid() && (!latest.isValid() || file.mtime > latest)) {
                latest = file.mtime;
            }
        }
        return latest;
    }
};

// ステータスを文字列に変換
inline QString diffStatusToString(DiffStatus status) {
    switch (status) {
        case DiffStatus::Match: return "一致";
        case DiffStatus::MatchMetadata: return "一致 (メタデータ)";
        case DiffStatus::Different: return "差異";
        case DiffStatus::MissingInA: return "Aに欠落";
        case DiffStatus::MissingInB: return "Bに欠落";
    }
    return "不明";
}

// ステータスに対応する色を取得
inline QColor diffStatusToColor(DiffStatus status) {
    switch (status) {
        case DiffStatus::Match:
        case DiffStatus::MatchMetadata:
            return QColor("#c8facc");  // 緑
        case DiffStatus::Different:
            return QColor("#fdb6b6");  // 赤
        case DiffStatus::MissingInA:
        case DiffStatus::MissingInB:
            return QColor("#dddddd");  // グレー
    }
    return QColor("#ffffff");
}

} // namespace DiffLoupe

#endif // DIFFRESULT_H