//
// Created by eightman on 25/07/13.
//
#include "mainwindow.h"
#include "ui_mainwindow.h"
#include "workers/compareworker.h"
#include "ui/diffviewer.h"
#include "ui/imageviewer.h"
#include "ui/hexviewer.h"
#include <QFileDialog>
#include <QFileInfo>
#include <QMessageBox>
#include <QShortcut>
#include <QKeySequence>
#include <QTreeWidgetItem>
#include <QProgressBar>
#include <QPushButton>
#include <QLabel>
#include <QComboBox>
#include <QCheckBox>
#include <QRadioButton>
#include <QButtonGroup>
#include <QAbstractButton>
#include <QStackedWidget>
#include <QSplitter>
#include <QHeaderView>
#include <QMenuBar>
#include <QMenu>
#include <QAction>
#include <QCloseEvent>
#include <QMenuBar>
#include <QMenu>
#include <QAction>
#include <QFile>
#include <QFileInfo>
#include <algorithm>

namespace DiffLoupe {

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent)
    , ui(std::make_unique<Ui::MainWindow>())
{
    ui->setupUi(this);
    setupUi();

    setupConnections();

    setupMenu();
    setupShortcuts();
}

MainWindow::~MainWindow()
{
    cleanupWorkers();
}

void MainWindow::setupUi()
{
    // UIコンポーネントの取得
    m_folderALabel = ui->folderALabel;
    m_folderBLabel = ui->folderBLabel;
    m_progressBar = ui->progressBar;
    m_fileCountLabel = ui->fileCountLabel;

    m_treeA = ui->treeA;
    m_treeB = ui->treeB;

    m_treeA->setHeaderLabels({"ファイル", "状態"});
    m_treeB->setHeaderLabels({"ファイル", "状態"});
    m_treeA->header()->setSectionResizeMode(QHeaderView::ResizeToContents);
    m_treeB->header()->setSectionResizeMode(QHeaderView::ResizeToContents);
    m_treeA->setAlternatingRowColors(true);
    m_treeB->setAlternatingRowColors(true);

    m_textModeBtn = ui->textModeBtn;
    m_imageModeBtn = ui->imageModeBtn;
    m_hexModeBtn = ui->hexModeBtn;
    m_diffModeBtn = ui->diffModeBtn;

    m_encodingCombo = ui->encodingCombo;
    m_sortCombo = ui->sortCombo;
    m_showHiddenCheckbox = ui->showHiddenCheckbox;

    m_filterAllRadio = ui->filterAllRadio;
    m_filterMissingRadio = ui->filterMissingRadio;
    m_filterDiffRadio = ui->filterDiffRadio;

    m_viewerStack = ui->viewerStack;

    // エンコーディングリストの設定
    m_encodingCombo->addItems({
        "Auto-detect", "UTF-8", "UTF-16", "UTF-32",
        "Shift_JIS", "EUC-JP", "CP932", "ISO-2022-JP",
        "Windows-1252", "ISO-8859-1", "Big5", "GB2312", "GBK",
        "KS_C_5601-1987", "ASCII"
    });

    // ソートオプションの設定
    m_sortCombo->addItems({
        "名前 (昇順)", "名前 (降順)",
        "更新日 (昇順)", "更新日 (降順)",
        "サイズ (昇順)", "サイズ (降順)"
    });

    // フィルターグループの設定
    m_filterGroup = new QButtonGroup(this);
    m_filterGroup->addButton(m_filterAllRadio, 0);
    m_filterGroup->addButton(m_filterMissingRadio, 1);
    m_filterGroup->addButton(m_filterDiffRadio, 2);

    // ビューアスタックの初期化
    m_viewerStack->addWidget(new DiffLoupe::DiffViewer(this));
    m_viewerStack->addWidget(new DiffLoupe::ImageViewer(this));
    m_viewerStack->addWidget(new DiffLoupe::HexViewer(this));
}


void MainWindow::setupConnections()
{
    // ボタンのクリック
    connect(ui->selectFolderABtn, &QPushButton::clicked,
            this, &MainWindow::onSelectFolderA);
    connect(ui->selectFolderBBtn, &QPushButton::clicked,
            this, &MainWindow::onSelectFolderB);
    connect(ui->compareBtn, &QPushButton::clicked,
            this, &MainWindow::onCompare);

    // ツリーアイテム選択
    connect(ui->treeA, &QTreeWidget::currentItemChanged,
            this, &MainWindow::onTreeItemSelected);
    connect(ui->treeB, &QTreeWidget::currentItemChanged,
            this, &MainWindow::onTreeItemSelected);

    // 表示モード
    connect(m_textModeBtn, &QPushButton::clicked,
            [this]() { onViewModeChanged(0); });
    connect(m_imageModeBtn, &QPushButton::clicked,
            [this]() { onViewModeChanged(1); });
    connect(m_hexModeBtn, &QPushButton::clicked,
            [this]() { onViewModeChanged(2); });
    
    // 差分モード
    connect(m_diffModeBtn, &QPushButton::toggled,
            this, &MainWindow::onDiffModeToggled);

    // 設定変更
    connect(m_encodingCombo, &QComboBox::currentTextChanged,
            this, &MainWindow::onEncodingChanged);
    connect(m_showHiddenCheckbox, &QCheckBox::toggled,
            this, &MainWindow::onShowHiddenChanged);
    connect(m_filterGroup, QOverload<QAbstractButton *>::of(&QButtonGroup::buttonClicked),
            [this](QAbstractButton *btn) {
                onFilterChanged(m_filterGroup->id(btn));
            });
    connect(ui->filterResetBtn, &QPushButton::clicked,
            this, &MainWindow::applyFilterAndRebuildTrees);
    connect(m_sortCombo, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, &MainWindow::onSortChanged);
}
void MainWindow::setupMenu()
{
    // "ヘルプ" メニューに "このソフトについて" を追加
    QMenu *helpMenu = menuBar()->addMenu(tr("&Help"));
    QAction *aboutAction = helpMenu->addAction(tr("このソフトについて..."));
    aboutAction->setMenuRole(QAction::AboutRole);
    connect(aboutAction, &QAction::triggered,
            this, &MainWindow::showAboutDialog);

    // 設定メニューの追加
    QMenu *settingsMenu = menuBar()->addMenu(tr("設定"));
    m_modernAction = settingsMenu->addAction(tr("モダンUIモード"));
    m_modernAction->setCheckable(true);
    connect(m_modernAction, &QAction::toggled,
            this, &MainWindow::toggleModernMode);
}

void MainWindow::showEvent(QShowEvent *event) {
    QMainWindow::showEvent(event);
    // スプリッターのサイズ設定
    ui->mainSplitter->setSizes({250, 900, 250});
}

void MainWindow::setupShortcuts()
{
    // Ctrl+1: すべて表示
    auto shortcutAll = new QShortcut(QKeySequence("Ctrl+1"), this);
    connect(shortcutAll, &QShortcut::activated, [this]() {
        m_filterAllRadio->setChecked(true);
        onFilterChanged(0);
    });

    // Ctrl+2: 欠落・新規のみ
    auto shortcutMissing = new QShortcut(QKeySequence("Ctrl+2"), this);
    connect(shortcutMissing, &QShortcut::activated, [this]() {
        m_filterMissingRadio->setChecked(true);
        onFilterChanged(1);
    });

    // Ctrl+3: 差分のみ
    auto shortcutDiff = new QShortcut(QKeySequence("Ctrl+3"), this);
    connect(shortcutDiff, &QShortcut::activated, [this]() {
        m_filterDiffRadio->setChecked(true);
        onFilterChanged(2);
    });

    // Ctrl+R: リセット
    auto shortcutReset = new QShortcut(QKeySequence("Ctrl+R"), this);
    connect(shortcutReset, &QShortcut::activated,
            ui->filterResetBtn, &QPushButton::click);

    // F5: 比較実行
    auto shortcutCompare = new QShortcut(QKeySequence("F5"), this);
    connect(shortcutCompare, &QShortcut::activated,
            ui->compareBtn, &QPushButton::click);

    // Ctrl+S: ソートメニューにフォーカス
    auto shortcutSort = new QShortcut(QKeySequence("Ctrl+S"), this);
    connect(shortcutSort, &QShortcut::activated,
            [this]() { m_sortCombo->setFocus(); });
}

void MainWindow::closeEvent(QCloseEvent *event)
{
    cleanupWorkers();
    event->accept();
}

void MainWindow::onSelectFolderA()
{
    QString folder = QFileDialog::getExistingDirectory(
        this, "フォルダAを選択", m_folderA);

    if (!folder.isEmpty()) {
        m_folderA = folder;
        m_folderALabel->setText(
            QString("フォルダA: %1").arg(QFileInfo(folder).fileName()));
    }
}

void MainWindow::onSelectFolderB()
{
    QString folder = QFileDialog::getExistingDirectory(
        this, "フォルダBを選択", m_folderB);

    if (!folder.isEmpty()) {
        m_folderB = folder;
        m_folderBLabel->setText(
            QString("フォルダB: %1").arg(QFileInfo(folder).fileName()));
    }
}

void MainWindow::onCompare()
{
    if (m_folderA.isEmpty() || m_folderB.isEmpty()) {
        statusBar()->showMessage("2つのフォルダを選択してください。");
        return;
    }

    ui->compareBtn->setEnabled(false);
    m_progressBar->setVisible(true);
    m_progressBar->setRange(0, 0); // 不確定進捗

    // 既存のワーカーを停止
    if (m_compareWorker && m_compareWorker->isRunning()) {
        m_compareWorker->quit();
        m_compareWorker->wait();
    }

    // 新しいワーカーを作成
    m_compareWorker = new CompareWorker(this);
    m_compareWorker->setFolders(m_folderA, m_folderB);

    Comparator::Settings settings;
    settings.showHidden = m_showHiddenCheckbox->isChecked();
    m_compareWorker->setSettings(settings);

    // シグナル接続
    connect(m_compareWorker, &CompareWorker::comparisonFinished,
            this, &MainWindow::onComparisonFinished);
    connect(m_compareWorker, &CompareWorker::progressUpdated,
            this, &MainWindow::onProgressUpdated);
    connect(m_compareWorker, &CompareWorker::finished,
            [this]() {
                ui->compareBtn->setEnabled(true);
                m_progressBar->setVisible(false);
            });

    m_compareWorker->start();
}

void MainWindow::onTreeItemSelected(QTreeWidgetItem *current, QTreeWidgetItem *previous)
{
    Q_UNUSED(previous)

    if (!current || current->childCount() > 0) {
        clearViewers();
        return;
    }

    auto it = m_itemMap.find(current);
    if (it == m_itemMap.end()) {
        return;
    }

    const DiffResult& result = it->second;

    QString filePathA;
    QString filePathB;
    if (result.files.size() > 0)
        filePathA = result.files[0].filePath;
    if (result.files.size() > 1)
        filePathB = result.files[1].filePath;

    loadFilesIntoViewer(filePathA, filePathB);
}

void MainWindow::onViewModeChanged(int mode)
{
    m_currentViewMode = mode;
    m_textModeBtn->setChecked(mode == 0);
    m_imageModeBtn->setChecked(mode == 1);
    m_hexModeBtn->setChecked(mode == 2);
    m_viewerStack->setCurrentIndex(mode);

    // 現在選択されているアイテムを再読み込み
    onTreeItemSelected(m_treeA->currentItem(), nullptr);
}

void MainWindow::onEncodingChanged(const QString &encoding)
{
    // 現在選択されているファイルを再読み込み
    onTreeItemSelected(m_treeA->currentItem(), nullptr);
}

void MainWindow::onFilterChanged(int filterId)
{
    m_currentFilter = filterId;
    if (!m_comparisonResults.empty()) {
        applyFilterAndRebuildTrees();
        statusBar()->showMessage(
            QString("フィルターを適用しました: %1").arg(getFilterName()));
    }
}

void MainWindow::onSortChanged(int index)
{
    m_currentSort = index;
    if (!m_comparisonResults.empty()) {
        applyFilterAndRebuildTrees();
        QString sortName = m_sortCombo->currentText();
        statusBar()->showMessage(
            QString("ソートを適用しました: %1").arg(sortName));
    }
}

void MainWindow::onShowHiddenChanged(bool checked)
{
    Q_UNUSED(checked)
    if (!m_folderA.isEmpty() && !m_folderB.isEmpty()) {
        statusBar()->showMessage(
            "隠しファイル設定が変更されました。再比較してください。");
    }
}

void MainWindow::onDiffModeToggled()
{
    bool isLineMode = m_diffModeBtn->isChecked();
    m_diffModeBtn->setText(isLineMode ? "行単位" : "文字単位");
    
    // Get the current diff viewer if text mode is active
    if (m_currentViewMode == 0) { // Text mode
        auto* diffViewer = qobject_cast<DiffLoupe::DiffViewer*>(m_viewerStack->widget(0));
        if (diffViewer) {
            diffViewer->setDiffMode(isLineMode ? DiffLoupe::DiffViewer::DiffMode::LINE 
                                               : DiffLoupe::DiffViewer::DiffMode::CHARACTER);
        }
    }
}

void MainWindow::onComparisonFinished(const std::vector<DiffResult> &results)
{
    m_comparisonResults = results;
    m_itemMap.clear();
    applyFilterAndRebuildTrees();
    statusBar()->showMessage("比較完了");
}

void MainWindow::onProgressUpdated(const QString &message)
{
    statusBar()->showMessage(message);
}

void MainWindow::clearTrees() {
    m_treeA->clear();
    m_treeB->clear();
    m_itemMap.clear();
}

void MainWindow::applyFilterAndRebuildTrees()
{
    clearTrees();

    std::vector<DiffResult> filteredResults;
    for (const auto& result : m_comparisonResults) {
        if (shouldIncludeFile(result)) {
            filteredResults.push_back(result);
        }
    }

    sortResults(filteredResults);

    populateTrees(filteredResults);

    m_fileCountLabel->setText(QString("%1 / %2 件").arg(filteredResults.size()).arg(m_comparisonResults.size()));
}

void MainWindow::populateTrees(const std::vector<DiffResult> &results) {
    auto createParentPath = [](QTreeWidget* tree, const QString& relativePath) -> QTreeWidgetItem* {
        QFileInfo fileInfo(relativePath);
        QString dirName = fileInfo.path();

        if (dirName.isEmpty() || dirName == ".") {
            return tree->invisibleRootItem();
        }

        QStringList parts = dirName.split('/', Qt::SkipEmptyParts);
        QTreeWidgetItem* currentParent = tree->invisibleRootItem();

        for (const QString& part : parts) {
            QTreeWidgetItem* childItem = nullptr;
            for (int i = 0; i < currentParent->childCount(); ++i) {
                if (currentParent->child(i)->text(0) == part) {
                    childItem = currentParent->child(i);
                    break;
                }
            }
            if (!childItem) {
                childItem = new QTreeWidgetItem(currentParent, QStringList(part));
            }
            currentParent = childItem;
        }
        return currentParent;
    };

    for (const auto& result : results) {
        QTreeWidgetItem* parentA = createParentPath(m_treeA, result.relativePath);
        QTreeWidgetItem* parentB = createParentPath(m_treeB, result.relativePath);

        QString fileName = QFileInfo(result.relativePath).fileName();
        QTreeWidgetItem* itemA = new QTreeWidgetItem(parentA, {fileName, diffStatusToString(result.status)});
        QTreeWidgetItem* itemB = new QTreeWidgetItem(parentB, {fileName, diffStatusToString(result.status)});

        styleTreeItem(itemA, result);
        styleTreeItem(itemB, result);

        m_itemMap[itemA] = result;
        m_itemMap[itemB] = result;
    }
}

void MainWindow::styleTreeItem(QTreeWidgetItem *item, const DiffResult &result)
{
    QColor color = diffStatusToColor(result.status);
    item->setBackground(0, color);
    item->setBackground(1, color);
    QString text = diffStatusToString(result.status);
    item->setToolTip(0, text);
    item->setToolTip(1, text);
}

bool MainWindow::shouldIncludeFile(const DiffResult &result) const {
    switch (m_currentFilter) {
        case 0: // All
            return true;
        case 1: // Missing
            return result.isMissing();
        case 2: // Different
            return result.isDifferent();
        default:
            return true;
    }
}

QString MainWindow::getFilterName() const {
    switch (m_currentFilter) {
        case 0: return "すべて";
        case 1: return "欠落・新規のみ";
        case 2: return "差分のみ";
        default: return "不明";
    }
}

void MainWindow::sortResults(std::vector<DiffResult> &results) const {
    // ソートキーの定義
    enum class SortKey {
        Name,
        Size,
        MTime
    };

    // ソート順序の定義
    enum class SortOrder {
        Ascending,
        Descending
    };

    // m_currentSortからソートキーと順序を決定
    SortKey key;
    SortOrder order;
    
    switch (m_currentSort) {
        case 0: // 名前 (昇順)
            key = SortKey::Name;
            order = SortOrder::Ascending;
            break;
        case 1: // 名前 (降順)
            key = SortKey::Name;
            order = SortOrder::Descending;
            break;
        case 2: // 更新日 (昇順)
            key = SortKey::MTime;
            order = SortOrder::Ascending;
            break;
        case 3: // 更新日 (降順)
            key = SortKey::MTime;
            order = SortOrder::Descending;
            break;
        case 4: // サイズ (昇順)
            key = SortKey::Size;
            order = SortOrder::Ascending;
            break;
        case 5: // サイズ (降順)
            key = SortKey::Size;
            order = SortOrder::Descending;
            break;
        default:
            return; // 無効なソート設定の場合はソートしない
    }

    // std::sortでソート実行
    std::sort(results.begin(), results.end(),
        [&](const DiffResult& a, const DiffResult& b) {
            bool less = false;
            
            switch (key) {
                case SortKey::Name: {
                    // QFileInfoを使ってファイル名を取得し、ロケールを考慮して比較
                    QString nameA = QFileInfo(a.relativePath).fileName();
                    QString nameB = QFileInfo(b.relativePath).fileName();
                    less = (QString::localeAwareCompare(nameA, nameB) < 0);
                    break;
                }
                case SortKey::Size: {
                    // ヘルパー関数で最大サイズを取得して比較
                    qint64 sizeA = a.getMaxSize();
                    qint64 sizeB = b.getMaxSize();
                    if (sizeA != sizeB) {
                        less = (sizeA < sizeB);
                    } else {
                        // サイズが同じ場合はファイル名で二次ソート
                        QString nameA = QFileInfo(a.relativePath).fileName();
                        QString nameB = QFileInfo(b.relativePath).fileName();
                        less = (QString::localeAwareCompare(nameA, nameB) < 0);
                    }
                    break;
                }
                case SortKey::MTime: {
                    // ヘルパー関数で最新の更新日時を取得して比較
                    QDateTime mtimeA = a.getLatestMTime();
                    QDateTime mtimeB = b.getLatestMTime();
                    
                    // 無効な日時の場合は古い扱いにする
                    if (!mtimeA.isValid() && !mtimeB.isValid()) {
                        // 両方とも無効な場合はファイル名で比較
                        QString nameA = QFileInfo(a.relativePath).fileName();
                        QString nameB = QFileInfo(b.relativePath).fileName();
                        less = (QString::localeAwareCompare(nameA, nameB) < 0);
                    } else if (!mtimeA.isValid()) {
                        less = true; // Aが無効な場合、Aを古い扱いにする
                    } else if (!mtimeB.isValid()) {
                        less = false; // Bが無効な場合、Bを古い扱いにする
                    } else if (mtimeA != mtimeB) {
                        less = (mtimeA < mtimeB);
                    } else {
                        // 更新日時が同じ場合はファイル名で二次ソート
                        QString nameA = QFileInfo(a.relativePath).fileName();
                        QString nameB = QFileInfo(b.relativePath).fileName();
                        less = (QString::localeAwareCompare(nameA, nameB) < 0);
                    }
                    break;
                }
            }

            // 昇順/降順の適用
            return (order == SortOrder::Ascending) ? less : !less;
        });
}

void MainWindow::cleanupWorkers()
{
    if (m_compareWorker && m_compareWorker->isRunning()) {
        m_compareWorker->quit();
        m_compareWorker->wait(5000);
        if (m_compareWorker->isRunning()) {
            m_compareWorker->terminate();
            m_compareWorker->wait();
        }
    }
}

// その他のメソッド実装は後続のステップで追加

void MainWindow::loadFilesIntoViewer(const QString &filePathA, const QString &filePathB) {
    if (m_viewerStack->currentIndex() == 0) { // DiffViewer
        static_cast<DiffViewer*>(m_viewerStack->widget(0))->setFiles(filePathA, filePathB);
        static_cast<DiffViewer*>(m_viewerStack->widget(0))->setEncoding(m_encodingCombo->currentText());
    } else if (m_viewerStack->currentIndex() == 1) { // ImageViewer
        static_cast<ImageViewer*>(m_viewerStack->widget(1))->setImages(filePathA, filePathB);
    } else if (m_viewerStack->currentIndex() == 2) { // HexViewer
        static_cast<HexViewer*>(m_viewerStack->widget(2))->setFile(filePathA);
    }
}

void MainWindow::clearViewers() {
    static_cast<DiffViewer*>(m_viewerStack->widget(0))->clear();
    static_cast<ImageViewer*>(m_viewerStack->widget(1))->clear();
    static_cast<HexViewer*>(m_viewerStack->widget(2))->clear();
}


void MainWindow::toggleModernMode(bool enabled)
{
    m_modernMode = enabled;
    if (enabled) {
        QFile f(":/modern_theme.qss");
        if (f.open(QIODevice::ReadOnly | QIODevice::Text)) {
            setStyleSheet(QString::fromUtf8(f.readAll()));
            f.close();
        } else {
            setStyleSheet("");
        }
    } else {
        setStyleSheet("");
    }
}

void MainWindow::showAboutDialog()
{
    QMessageBox::about(this, tr("このソフトについて"),
                       tr("DiffLoupe\nフォルダ・ファイル比較ツール"));
}

} // namespace DiffLoupe

