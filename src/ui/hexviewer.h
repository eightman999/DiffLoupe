#ifndef HEXVIEWER_H
#define HEXVIEWER_H

#include <QWidget>
#include <QTextEdit>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QSplitter>
#include <QScrollBar>
#include <QFile>
#include <QByteArray>
#include <QList>
#include <QColor>

namespace DiffLoupe {

// 差分の種類を定義
enum class DiffType { None, Modified, Inserted, Deleted };

// 差分情報を保持する構造体
struct DiffInfo {
    DiffType type;
    int start;  // 元データでの開始オフセット
    int length; // 長さ
};

class HexViewer : public QWidget {
    Q_OBJECT

public:
    explicit HexViewer(QWidget *parent = nullptr);
    ~HexViewer();

    void setFile(const QString &file);
    void setFiles(const QString &fileA, const QString &fileB);
    void clear();

private slots:
    void syncScroll1(int value);
    void syncScroll2(int value);

private:
    void setupUI();
    void updateSingleView();
    void performComparison();
    void updateDiffViews();
    void applyHighlighting(QTextEdit* textEdit, const QByteArray& data, const QList<DiffInfo>& diffs, bool isLeftView);
    QString generateHexDisplay(const QByteArray& data, bool isLeft);
    QString formatHexLine(int offset, const QByteArray& lineData, int highlightStart = -1, int highlightLen = 0, const QColor& color = QColor());

    // UI Components
    QSplitter *m_splitter;
    QTextEdit *m_leftTextEdit;
    QTextEdit *m_rightTextEdit;
    QTextEdit *m_singleTextEdit;  // 単一ファイル表示用

    // File data
    QString m_filePathA;
    QString m_filePathB;
    QByteArray m_dataA;
    QByteArray m_dataB;
    QList<DiffInfo> m_diffs;

    // State
    bool m_isDiffMode = false;
    bool m_syncingScroll = false;
};

} // namespace DiffLoupe

#endif // HEXVIEWER_H
