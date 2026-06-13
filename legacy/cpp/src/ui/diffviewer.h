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

// テキスト差分を表示するウィジェット
class DiffViewer : public QWidget {
    Q_OBJECT

public:
    /** 差分表示モード */
    enum class DiffMode {
        LINE,      //!< 行単位で比較
        CHARACTER  //!< 文字単位で比較
    };

    /**
     * @brief コンストラクタ
     * @param parent 親ウィジェット
     */
    explicit DiffViewer(QWidget *parent = nullptr);

    /** デストラクタ */
    ~DiffViewer();

    /**
     * @brief 表示するファイルを設定
     * @param fileA 左側のファイルパス
     * @param fileB 右側のファイルパス
     */
    void setFiles(const QString &fileA, const QString &fileB);

    /**
     * @brief 読み込みエンコーディングを設定
     */
    void setEncoding(const QString &encoding);

    /**
     * @brief 差分モードを設定
     */
    void setDiffMode(DiffMode mode);

    /** 現在の差分モードを取得 */
    DiffMode getDiffMode() const { return m_diffMode; }

    /** 表示内容をクリア */
    void clear();

private:
    /** UIコンポーネントを保持 */
    QSplitter *m_splitter;      //!< 左右エディタを分割するスプリッター
    QTextEdit *m_leftEditor;    //!< 左側エディタ（ファイルA用）
    QTextEdit *m_rightEditor;   //!< 右側エディタ（ファイルB用）

    /** 読み込むファイル情報 */
    QString m_fileA;      //!< 左側ファイルパス
    QString m_fileB;      //!< 右側ファイルパス
    QString m_encoding;   //!< テキストエンコーディング
    QString m_contentA;   //!< 左側ファイル内容
    QString m_contentB;   //!< 右側ファイル内容

    /** ファイル読み込みワーカー */
    FileLoadWorker *m_workerA = nullptr; //!< 左側読み込みワーカー
    FileLoadWorker *m_workerB = nullptr; //!< 右側読み込みワーカー

    /** 現在の差分モード */
    DiffMode m_diffMode = DiffMode::LINE; //!< デフォルトは行単位

    /** 内部処理メソッド */
    void setupUI();
    void startLoading();
    void updateDiffView();
    void processDiffs(const std::list<diff_match_patch<std::wstring>::Diff>& diffs);
    void highlightDifferences(const std::list<diff_match_patch<std::wstring>::Diff>& diffs);
    void setupSyncScroll();
    std::list<diff_match_patch<std::wstring>::Diff> createLineDiff(const std::wstring& contentA,
                                                                  const std::wstring& contentB);
};

} // namespace DiffLoupe

#endif // DIFFVIEWER_H
