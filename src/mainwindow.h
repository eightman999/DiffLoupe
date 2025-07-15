//
// Created by eightman on 25/07/13.
//
#ifndef MAINWINDOW_H
#define MAINWINDOW_H

#include <QMainWindow>
#include <QPushButton>
#include <memory>
#include <vector>
#include <map>

QT_BEGIN_NAMESPACE
class QTreeWidget;
class QTreeWidgetItem;
class QProgressBar;
class QLabel;
class QComboBox;
class QCheckBox;
class QRadioButton;
class QButtonGroup;
class QStackedWidget;
class QSplitter;
class QMenu;
class QAction;
QT_END_NAMESPACE

namespace DiffLoupe {

class CompareWorker;
class DiffResult;

namespace Ui {
class MainWindow;
}

class MainWindow : public QMainWindow
{
    Q_OBJECT

public:
    explicit MainWindow(QWidget *parent = nullptr);
    ~MainWindow();

protected:
    void closeEvent(QCloseEvent *event) override;
    void showEvent(QShowEvent *event) override;

private slots:
    // フォルダ選択
    void onSelectFolderA();
    void onSelectFolderB();
    
    // 比較実行
    void onCompare();
    
    // ツリービューアイテム選択
    void onTreeItemSelected(QTreeWidgetItem *current, QTreeWidgetItem *previous);
    
    // 表示モード変更
    void onViewModeChanged(int mode);
    
    // エンコーディング変更
    void onEncodingChanged(const QString &encoding);
    
    // フィルター変更
    void onFilterChanged(int filterId);
    
    // ソート変更
    void onSortChanged(int index);
    
    // 隠しファイル表示切り替え
    void onShowHiddenChanged(bool checked);
    
    // 差分モード切り替え
    void onDiffModeToggled();

    // "このソフトについて" ダイアログ表示
    void showAboutDialog();
    
    // ワーカースレッドからの結果受信
    void onComparisonFinished(const std::vector<DiffResult> &results);
    void onProgressUpdated(const QString &message);
    
private:
    // UI初期化
    void setupUi();
    void setupConnections();
    void setupShortcuts();
    void setupMenu();
    
    // ツリー関連
    void populateTrees(const std::vector<DiffResult> &results);
    void clearTrees();
    void applyFilterAndRebuildTrees();
    void styleTreeItem(QTreeWidgetItem *item, const DiffResult &result);
    
    // ビューア関連
    void loadFilesIntoViewer(const QString &filePathA, const QString &filePathB);
    void clearViewers();
    
    // フィルター関連
    bool shouldIncludeFile(const DiffResult &result) const;
    QString getFilterName() const;
    
    // ソート関連
    void sortResults(std::vector<DiffResult> &results) const;
    
    // ワーカースレッド管理
    void cleanupWorkers();
    void toggleModernMode(bool enabled);
    
private:
    std::unique_ptr<Ui::MainWindow> ui;
    
    // フォルダパス
    QString m_folderA;
    QString m_folderB;
    
    // UI要素
    QLabel *m_folderALabel;
    QLabel *m_folderBLabel;
    QProgressBar *m_progressBar;
    QLabel *m_fileCountLabel;
    
    // ツリービュー
    QTreeWidget *m_treeA;
    QTreeWidget *m_treeB;
    
    // 表示モードボタン
    QPushButton *m_textModeBtn;
    QPushButton *m_imageModeBtn;
    QPushButton *m_hexModeBtn;
    
    // 差分モードボタン
    QPushButton *m_diffModeBtn;
    
    // エンコーディング選択
    QComboBox *m_encodingCombo;
    
    // フィルター
    QRadioButton *m_filterAllRadio;
    QRadioButton *m_filterMissingRadio;
    QRadioButton *m_filterDiffRadio;
    QButtonGroup *m_filterGroup;
    
    // ソート
    QComboBox *m_sortCombo;
    
    // その他の設定
    QCheckBox *m_showHiddenCheckbox;

    // メニュー
    QAction *m_modernAction = nullptr;
    bool m_modernMode = false;
    
    // ビューアスタック
    QStackedWidget *m_viewerStack;
    
    // 比較結果
    std::vector<DiffResult> m_comparisonResults;
    std::map<QTreeWidgetItem*, DiffResult> m_itemMap;
    
    // 現在の設定
    int m_currentFilter = 0;
    int m_currentSort = 0;
    int m_currentViewMode = 0;
    
    // ワーカースレッド
    CompareWorker *m_compareWorker = nullptr;
    
    // 定数
    static constexpr const char* IMAGE_EXTENSIONS[] = {
        ".png", ".jpg", ".jpeg", ".bmp", ".tga", ".dds", ".gif", ".tiff"
    };
};

} // namespace DiffLoupe

#endif // MAINWINDOW_H