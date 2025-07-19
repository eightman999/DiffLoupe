// Copyright (c) eightman 2005-2025
// Furin-lab All rights reserved.
// 動作設計: DiffViewerクラス宣言。テキスト差分の表示UIを構築

#ifndef DIFFVIEWER_H
#define DIFFVIEWER_H

#include <QWidget>
#include <QTextEdit>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QSplitter>
#include <QScrollBar>
#include <QFile>
#include <QTextStream>
#include <QTextCodec>
#include "workers/fileloadworker.h"
#include "core/diff_match_patch.h"

namespace DiffLoupe {

class DiffViewer : public QWidget {
    Q_OBJECT

public:
    enum class DiffMode {
        LINE,      // Line-based comparison (default)
        CHARACTER  // Character-based comparison
    };

    explicit DiffViewer(QWidget *parent = nullptr);
    ~DiffViewer();

    void setFiles(const QString &fileA, const QString &fileB);
    void setEncoding(const QString &encoding);
    void setDiffMode(DiffMode mode);
    DiffMode getDiffMode() const { return m_diffMode; }
    void clear();

private:
    // UI Components
    QSplitter *m_splitter;
    QTextEdit *m_leftEditor;
    QTextEdit *m_rightEditor;
    
    // File data
    QString m_fileA;
    QString m_fileB;
    QString m_encoding;
    QString m_contentA;
    QString m_contentB;
    
    // Workers
    FileLoadWorker *m_workerA = nullptr;
    FileLoadWorker *m_workerB = nullptr;
    
    // Diff mode
    DiffMode m_diffMode = DiffMode::LINE; // Default to line-based comparison

    // Methods
    void setupUI();
    void startLoading();
    void updateDiffView();
    void processDiffs(const std::list<diff_match_patch<std::wstring>::Diff>& diffs);
    void highlightDifferences(const std::list<diff_match_patch<std::wstring>::Diff>& diffs);
    void setupSyncScroll();
    std::list<diff_match_patch<std::wstring>::Diff> createLineDiff(const std::wstring& contentA, const std::wstring& contentB);
};

} // namespace DiffLoupe

#endif // DIFFVIEWER_H
