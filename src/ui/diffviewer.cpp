#include "diffviewer.h"
#include "workers/fileloadworker.h"
#include "core/diff_match_patch.h"
#include <algorithm>
#include <sstream>
#include <map>
#include <vector>

namespace DiffLoupe {

DiffViewer::DiffViewer(QWidget *parent) : QWidget(parent) {
    setupUI();
}

DiffViewer::~DiffViewer() = default;

void DiffViewer::setupUI() {
    // メインレイアウトの作成
    QHBoxLayout *mainLayout = new QHBoxLayout(this);
    mainLayout->setContentsMargins(0, 0, 0, 0);

    // スプリッターの作成
    m_splitter = new QSplitter(Qt::Horizontal, this);

    // 左側のエディタ（ファイルA）
    m_leftEditor = new QTextEdit(this);
    m_leftEditor->setReadOnly(true);
    m_leftEditor->setLineWrapMode(QTextEdit::NoWrap);
    m_leftEditor->setFont(QFont("Consolas", 10));
    m_leftEditor->setPlaceholderText("ファイルA");

    // 右側のエディタ（ファイルB）
    m_rightEditor = new QTextEdit(this);
    m_rightEditor->setReadOnly(true);
    m_rightEditor->setLineWrapMode(QTextEdit::NoWrap);
    m_rightEditor->setFont(QFont("Consolas", 10));
    m_rightEditor->setPlaceholderText("ファイルB");

    // スプリッターに追加
    m_splitter->addWidget(m_leftEditor);
    m_splitter->addWidget(m_rightEditor);
    m_splitter->setSizes({50, 50}); // 左右を均等に分割

    // メインレイアウトに追加
    mainLayout->addWidget(m_splitter);
    setLayout(mainLayout);

    // 同期スクロールの設定
    setupSyncScroll();
}

void DiffViewer::setupSyncScroll() {
    // 垂直スクロールの同期
    connect(m_leftEditor->verticalScrollBar(), &QScrollBar::valueChanged,
            m_rightEditor->verticalScrollBar(), &QScrollBar::setValue);
    connect(m_rightEditor->verticalScrollBar(), &QScrollBar::valueChanged,
            m_leftEditor->verticalScrollBar(), &QScrollBar::setValue);

    // 水平スクロールの同期
    connect(m_leftEditor->horizontalScrollBar(), &QScrollBar::valueChanged,
            m_rightEditor->horizontalScrollBar(), &QScrollBar::setValue);
    connect(m_rightEditor->horizontalScrollBar(), &QScrollBar::valueChanged,
            m_leftEditor->horizontalScrollBar(), &QScrollBar::setValue);
}

void DiffViewer::setFiles(const QString &fileA, const QString &fileB) {
    m_fileA = fileA;
    m_fileB = fileB;
    startLoading();
}

void DiffViewer::setEncoding(const QString &encoding) {
    m_encoding = encoding;
    startLoading();
}

void DiffViewer::setDiffMode(DiffMode mode) {
    if (m_diffMode != mode) {
        m_diffMode = mode;
        // Refresh the diff view with the new mode
        updateDiffView();
    }
}

void DiffViewer::clear() {
    m_leftEditor->clear();
    m_rightEditor->clear();
    m_fileA.clear();
    m_fileB.clear();
    m_contentA.clear();
    m_contentB.clear();
    if (m_workerA) {
        m_workerA->quit();
        m_workerA->wait();
        delete m_workerA;
        m_workerA = nullptr;
    }
    if (m_workerB) {
        m_workerB->quit();
        m_workerB->wait();
        delete m_workerB;
        m_workerB = nullptr;
    }
}

void DiffViewer::startLoading() {
    m_leftEditor->clear();
    m_rightEditor->clear();

    if (m_fileA.isEmpty() && m_fileB.isEmpty()) {
        return;
    }

    if (m_workerA) { m_workerA->quit(); m_workerA->wait(); delete m_workerA; m_workerA = nullptr; }
    if (m_workerB) { m_workerB->quit(); m_workerB->wait(); delete m_workerB; m_workerB = nullptr; }

    if (!m_fileA.isEmpty()) {
        m_workerA = new FileLoadWorker(this);
        m_workerA->setFilePath(m_fileA);
        m_workerA->setEncoding(m_encoding);
        connect(m_workerA, &FileLoadWorker::fileLoaded, this, [this](const QString &c){ m_contentA = c; updateDiffView(); });
        m_workerA->start();
    } else {
        m_contentA.clear();
    }

    if (!m_fileB.isEmpty()) {
        m_workerB = new FileLoadWorker(this);
        m_workerB->setFilePath(m_fileB);
        m_workerB->setEncoding(m_encoding);
        connect(m_workerB, &FileLoadWorker::fileLoaded, this, [this](const QString &c){ m_contentB = c; updateDiffView(); });
        m_workerB->start();
    } else {
        m_contentB.clear();
    }

    if (!m_workerA && !m_workerB) {
        updateDiffView();
    }
}

void DiffViewer::updateDiffView() {
    if ((m_fileA.isEmpty() || !m_workerA || !m_workerA->isRunning()) &&
        (m_fileB.isEmpty() || !m_workerB || !m_workerB->isRunning())) {
        
        // Myers差分アルゴリズムを使用した高度な差分表示
        diff_match_patch<std::wstring> dmp;
        
        // QStringからstd::wstringに変換
        std::wstring contentA = m_contentA.toStdWString();
        std::wstring contentB = m_contentB.toStdWString();
        
        std::list<diff_match_patch<std::wstring>::Diff> diffs;
        
        if (m_diffMode == DiffMode::LINE) {
            // Line-based diff processing
            // Create a custom line-based diff since the protected methods are not accessible
            diffs = createLineDiff(contentA, contentB);
        } else {
            // Character-based diff processing (original implementation)
            diffs = dmp.diff_main(contentA, contentB, false);
        }
        
        // 差分を読みやすくする
        dmp.diff_cleanupSemantic(diffs);
        
        // 左右表示用の差分処理
        processDiffs(diffs);
    }
}

void DiffViewer::processDiffs(const std::list<diff_match_patch<std::wstring>::Diff>& diffs) {
    QStringList leftLines, rightLines;
    
    for (const auto& diff : diffs) {
        QString text = QString::fromStdWString(diff.text);
        
        if (m_diffMode == DiffMode::LINE) {
            // Line-based mode: text already contains complete lines
            QStringList lines = text.split('\n', Qt::KeepEmptyParts);
            // For line mode, preserve all lines including empty ones
            if (!lines.isEmpty() && lines.last().isEmpty()) {
                lines.removeLast();
            }
            
            if (diff.operation == diff_match_patch<std::wstring>::EQUAL) {
                leftLines.append(lines);
                rightLines.append(lines);
            } else if (diff.operation == diff_match_patch<std::wstring>::DELETE) {
                leftLines.append(lines);
                for (int i = 0; i < lines.count(); ++i) {
                    rightLines.append(""); // 右側は空行
                }
            } else if (diff.operation == diff_match_patch<std::wstring>::INSERT) {
                rightLines.append(lines);
                for (int i = 0; i < lines.count(); ++i) {
                    leftLines.append(""); // 左側は空行
                }
            }
        } else {
            // Character-based mode: split text into lines for display
            QStringList lines = text.split('\n');
            
            // 最後の空行は不要な場合があるので削除
            if (!lines.isEmpty() && lines.last().isEmpty()) {
                lines.removeLast();
            }

            if (diff.operation == diff_match_patch<std::wstring>::EQUAL) {
                leftLines.append(lines);
                rightLines.append(lines);
            } else if (diff.operation == diff_match_patch<std::wstring>::DELETE) {
                leftLines.append(lines);
                for (int i = 0; i < lines.count(); ++i) {
                    rightLines.append(""); // 右側は空行
                }
            } else if (diff.operation == diff_match_patch<std::wstring>::INSERT) {
                rightLines.append(lines);
                for (int i = 0; i < lines.count(); ++i) {
                    leftLines.append(""); // 左側は空行
                }
            }
        }
    }

    // テキストを設定
    m_leftEditor->setPlainText(leftLines.join('\n'));
    m_rightEditor->setPlainText(rightLines.join('\n'));
    
    // 差分ハイライトを適用
    highlightDifferences(diffs);
}

void DiffViewer::highlightDifferences(const std::list<diff_match_patch<std::wstring>::Diff>& diffs) {
    QList<QTextEdit::ExtraSelection> leftSelections, rightSelections;
    
    // 差分ハイライト用の色
    QColor addedColor = QColor(220, 255, 220);    // 薄い緑
    QColor removedColor = QColor(255, 220, 220);  // 薄い赤
    
    int leftLineNum = 0;
    int rightLineNum = 0;
    
    for (const auto& diff : diffs) {
        QString text = QString::fromStdWString(diff.text);
        QStringList lines = text.split('\n');
        
        // 最後の空行は不要な場合があるので削除
        if (!lines.isEmpty() && lines.last().isEmpty()) {
            lines.removeLast();
        }

        if (diff.operation == diff_match_patch<std::wstring>::EQUAL) {
            // 変更がない行はそのまま進む
            leftLineNum += lines.count();
            rightLineNum += lines.count();
        } else if (diff.operation == diff_match_patch<std::wstring>::DELETE) {
            // 削除された行は左側をハイライト
            for (int i = 0; i < lines.count(); ++i) {
                QTextEdit::ExtraSelection sel;
                QTextCursor cursor(m_leftEditor->document());
                cursor.movePosition(QTextCursor::Start);
                cursor.movePosition(QTextCursor::Down, QTextCursor::MoveAnchor, leftLineNum + i);
                cursor.select(QTextCursor::LineUnderCursor);
                sel.cursor = cursor;
                sel.format.setBackground(removedColor);
                sel.format.setProperty(QTextFormat::FullWidthSelection, true);
                leftSelections.append(sel);
            }
            leftLineNum += lines.count();
            rightLineNum += lines.count(); // 右側は空行分進む
        } else if (diff.operation == diff_match_patch<std::wstring>::INSERT) {
            // 追加された行は右側をハイライト
            for (int i = 0; i < lines.count(); ++i) {
                QTextEdit::ExtraSelection sel;
                QTextCursor cursor(m_rightEditor->document());
                cursor.movePosition(QTextCursor::Start);
                cursor.movePosition(QTextCursor::Down, QTextCursor::MoveAnchor, rightLineNum + i);
                cursor.select(QTextCursor::LineUnderCursor);
                sel.cursor = cursor;
                sel.format.setBackground(addedColor);
                sel.format.setProperty(QTextFormat::FullWidthSelection, true);
                rightSelections.append(sel);
            }
            leftLineNum += lines.count(); // 左側は空行分進む
            rightLineNum += lines.count();
        }
    }

    // ハイライトを適用
    m_leftEditor->setExtraSelections(leftSelections);
    m_rightEditor->setExtraSelections(rightSelections);
}

std::list<diff_match_patch<std::wstring>::Diff> DiffViewer::createLineDiff(const std::wstring& contentA, const std::wstring& contentB) {
    // Split content into lines
    std::vector<std::wstring> linesA, linesB;
    
    // Split contentA into lines
    std::wstring::size_type start = 0;
    std::wstring::size_type pos = 0;
    while ((pos = contentA.find(L'\n', start)) != std::wstring::npos) {
        linesA.push_back(contentA.substr(start, pos - start + 1)); // Include the newline
        start = pos + 1;
    }
    if (start < contentA.length()) {
        linesA.push_back(contentA.substr(start)); // Last line without newline
    }
    
    // Split contentB into lines
    start = 0;
    pos = 0;
    while ((pos = contentB.find(L'\n', start)) != std::wstring::npos) {
        linesB.push_back(contentB.substr(start, pos - start + 1)); // Include the newline
        start = pos + 1;
    }
    if (start < contentB.length()) {
        linesB.push_back(contentB.substr(start)); // Last line without newline
    }
    
    // Create mapping from lines to characters
    std::map<std::wstring, wchar_t> lineToChar;
    std::vector<std::wstring> charToLine;
    wchar_t nextChar = 1; // Start from 1 to avoid null character
    
    // Build character mapping for both sets of lines
    auto addLineToMapping = [&](const std::wstring& line) -> wchar_t {
        auto it = lineToChar.find(line);
        if (it != lineToChar.end()) {
            return it->second;
        }
        wchar_t ch = nextChar++;
        lineToChar[line] = ch;
        if (charToLine.size() <= static_cast<size_t>(ch)) {
            charToLine.resize(ch + 1);
        }
        charToLine[ch] = line;
        return ch;
    };
    
    // Convert lines to character representation
    std::wstring charStringA, charStringB;
    for (const auto& line : linesA) {
        charStringA += addLineToMapping(line);
    }
    for (const auto& line : linesB) {
        charStringB += addLineToMapping(line);
    }
    
    // Perform character-based diff on the line-encoded strings
    diff_match_patch<std::wstring> dmp;
    auto charDiffs = dmp.diff_main(charStringA, charStringB, false);
    
    // Convert character diffs back to line diffs
    std::list<diff_match_patch<std::wstring>::Diff> lineDiffs;
    for (const auto& diff : charDiffs) {
        std::wstring lineText;
        for (wchar_t ch : diff.text) {
            if (ch > 0 && ch < static_cast<wchar_t>(charToLine.size())) {
                lineText += charToLine[ch];
            }
        }
        lineDiffs.push_back(diff_match_patch<std::wstring>::Diff(diff.operation, lineText));
    }
    
    return lineDiffs;
}

} // namespace DiffLoupe
