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
#include <QCloseEvent>

namespace DiffLoupe {

MainWindow::MainWindow(QWidget *parent)
    : QMainWindow(parent)
    , ui(std::make_unique<Ui::MainWindow>())
{
    ui->setupUi(this);
    setupUi();
    setupConnections();
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
    // TODO: ソートロジックを実装
    // m_currentSort の値に基づいてソート
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

} // namespace DiffLoupe