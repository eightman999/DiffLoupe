/********************************************************************************
** Form generated from reading UI file 'mainwindow.ui'
**
** Created by: Qt User Interface Compiler version 6.9.0
**
** WARNING! All changes made in this file will be lost when recompiling UI file!
********************************************************************************/

#ifndef UI_MAINWINDOW_H
#define UI_MAINWINDOW_H

#include <QtCore/QVariant>
#include <QtWidgets/QApplication>
#include <QtWidgets/QCheckBox>
#include <QtWidgets/QComboBox>
#include <QtWidgets/QHBoxLayout>
#include <QtWidgets/QHeaderView>
#include <QtWidgets/QLabel>
#include <QtWidgets/QMainWindow>
#include <QtWidgets/QMenuBar>
#include <QtWidgets/QProgressBar>
#include <QtWidgets/QPushButton>
#include <QtWidgets/QRadioButton>
#include <QtWidgets/QSpacerItem>
#include <QtWidgets/QSplitter>
#include <QtWidgets/QStackedWidget>
#include <QtWidgets/QStatusBar>
#include <QtWidgets/QTreeWidget>
#include <QtWidgets/QVBoxLayout>
#include <QtWidgets/QWidget>

namespace DiffLoupe {

class Ui_MainWindow
{
public:
    QWidget *centralwidget;
    QVBoxLayout *mainLayout;
    QHBoxLayout *topControlsLayout;
    QPushButton *selectFolderABtn;
    QLabel *folderALabel;
    QSpacerItem *horizontalSpacer1;
    QPushButton *selectFolderBBtn;
    QLabel *folderBLabel;
    QSpacerItem *horizontalSpacer2;
    QPushButton *compareBtn;
    QHBoxLayout *settingsRow1;
    QLabel *viewModeLabel;
    QPushButton *textModeBtn;
    QPushButton *imageModeBtn;
    QPushButton *hexModeBtn;
    QLabel *separator1;
    QLabel *encodingLabel;
    QComboBox *encodingCombo;
    QLabel *separator2;
    QCheckBox *showHiddenCheckbox;
    QSpacerItem *horizontalSpacer3;
    QHBoxLayout *settingsRow2;
    QLabel *filterLabel;
    QRadioButton *filterAllRadio;
    QRadioButton *filterMissingRadio;
    QRadioButton *filterDiffRadio;
    QLabel *separator3;
    QPushButton *filterResetBtn;
    QLabel *separator4;
    QLabel *fileCountLabel;
    QLabel *separator5;
    QLabel *sortLabel;
    QComboBox *sortCombo;
    QSpacerItem *horizontalSpacer4;
    QProgressBar *progressBar;
    QSplitter *mainSplitter;
    QTreeWidget *treeA;
    QStackedWidget *viewerStack;
    QTreeWidget *treeB;
    QMenuBar *menubar;
    QStatusBar *statusbar;

    void setupUi(QMainWindow *DiffLoupe__MainWindow)
    {
        if (DiffLoupe__MainWindow->objectName().isEmpty())
            DiffLoupe__MainWindow->setObjectName("DiffLoupe__MainWindow");
        DiffLoupe__MainWindow->resize(1400, 900);
        centralwidget = new QWidget(DiffLoupe__MainWindow);
        centralwidget->setObjectName("centralwidget");
        mainLayout = new QVBoxLayout(centralwidget);
        mainLayout->setSpacing(3);
        mainLayout->setContentsMargins(5, 5, 5, 5);
        mainLayout->setObjectName("mainLayout");
        topControlsLayout = new QHBoxLayout();
        topControlsLayout->setObjectName("topControlsLayout");
        selectFolderABtn = new QPushButton(centralwidget);
        selectFolderABtn->setObjectName("selectFolderABtn");

        topControlsLayout->addWidget(selectFolderABtn);

        folderALabel = new QLabel(centralwidget);
        folderALabel->setObjectName("folderALabel");

        topControlsLayout->addWidget(folderALabel);

        horizontalSpacer1 = new QSpacerItem(0, 0, QSizePolicy::Policy::Expanding, QSizePolicy::Policy::Minimum);

        topControlsLayout->addItem(horizontalSpacer1);

        selectFolderBBtn = new QPushButton(centralwidget);
        selectFolderBBtn->setObjectName("selectFolderBBtn");

        topControlsLayout->addWidget(selectFolderBBtn);

        folderBLabel = new QLabel(centralwidget);
        folderBLabel->setObjectName("folderBLabel");

        topControlsLayout->addWidget(folderBLabel);

        horizontalSpacer2 = new QSpacerItem(0, 0, QSizePolicy::Policy::Expanding, QSizePolicy::Policy::Minimum);

        topControlsLayout->addItem(horizontalSpacer2);

        compareBtn = new QPushButton(centralwidget);
        compareBtn->setObjectName("compareBtn");

        topControlsLayout->addWidget(compareBtn);


        mainLayout->addLayout(topControlsLayout);

        settingsRow1 = new QHBoxLayout();
        settingsRow1->setSpacing(8);
        settingsRow1->setObjectName("settingsRow1");
        viewModeLabel = new QLabel(centralwidget);
        viewModeLabel->setObjectName("viewModeLabel");

        settingsRow1->addWidget(viewModeLabel);

        textModeBtn = new QPushButton(centralwidget);
        textModeBtn->setObjectName("textModeBtn");
        textModeBtn->setCheckable(true);
        textModeBtn->setChecked(true);
        textModeBtn->setMaximumSize(QSize(60, 16777215));

        settingsRow1->addWidget(textModeBtn);

        imageModeBtn = new QPushButton(centralwidget);
        imageModeBtn->setObjectName("imageModeBtn");
        imageModeBtn->setCheckable(true);
        imageModeBtn->setMaximumSize(QSize(60, 16777215));

        settingsRow1->addWidget(imageModeBtn);

        hexModeBtn = new QPushButton(centralwidget);
        hexModeBtn->setObjectName("hexModeBtn");
        hexModeBtn->setCheckable(true);
        hexModeBtn->setMaximumSize(QSize(60, 16777215));

        settingsRow1->addWidget(hexModeBtn);

        separator1 = new QLabel(centralwidget);
        separator1->setObjectName("separator1");

        settingsRow1->addWidget(separator1);

        encodingLabel = new QLabel(centralwidget);
        encodingLabel->setObjectName("encodingLabel");

        settingsRow1->addWidget(encodingLabel);

        encodingCombo = new QComboBox(centralwidget);
        encodingCombo->setObjectName("encodingCombo");
        encodingCombo->setMaximumSize(QSize(120, 16777215));

        settingsRow1->addWidget(encodingCombo);

        separator2 = new QLabel(centralwidget);
        separator2->setObjectName("separator2");

        settingsRow1->addWidget(separator2);

        showHiddenCheckbox = new QCheckBox(centralwidget);
        showHiddenCheckbox->setObjectName("showHiddenCheckbox");

        settingsRow1->addWidget(showHiddenCheckbox);

        horizontalSpacer3 = new QSpacerItem(0, 0, QSizePolicy::Policy::Expanding, QSizePolicy::Policy::Minimum);

        settingsRow1->addItem(horizontalSpacer3);


        mainLayout->addLayout(settingsRow1);

        settingsRow2 = new QHBoxLayout();
        settingsRow2->setSpacing(8);
        settingsRow2->setContentsMargins(0, 0, 0, 0);
        settingsRow2->setObjectName("settingsRow2");
        filterLabel = new QLabel(centralwidget);
        filterLabel->setObjectName("filterLabel");

        settingsRow2->addWidget(filterLabel);

        filterAllRadio = new QRadioButton(centralwidget);
        filterAllRadio->setObjectName("filterAllRadio");
        filterAllRadio->setChecked(true);
        filterAllRadio->setMaximumSize(QSize(16777215, 25));

        settingsRow2->addWidget(filterAllRadio);

        filterMissingRadio = new QRadioButton(centralwidget);
        filterMissingRadio->setObjectName("filterMissingRadio");
        filterMissingRadio->setMaximumSize(QSize(16777215, 25));

        settingsRow2->addWidget(filterMissingRadio);

        filterDiffRadio = new QRadioButton(centralwidget);
        filterDiffRadio->setObjectName("filterDiffRadio");
        filterDiffRadio->setMaximumSize(QSize(16777215, 25));

        settingsRow2->addWidget(filterDiffRadio);

        separator3 = new QLabel(centralwidget);
        separator3->setObjectName("separator3");

        settingsRow2->addWidget(separator3);

        filterResetBtn = new QPushButton(centralwidget);
        filterResetBtn->setObjectName("filterResetBtn");
        filterResetBtn->setMaximumSize(QSize(60, 25));

        settingsRow2->addWidget(filterResetBtn);

        separator4 = new QLabel(centralwidget);
        separator4->setObjectName("separator4");

        settingsRow2->addWidget(separator4);

        fileCountLabel = new QLabel(centralwidget);
        fileCountLabel->setObjectName("fileCountLabel");
        fileCountLabel->setMinimumSize(QSize(100, 0));
        fileCountLabel->setMaximumSize(QSize(16777215, 25));

        settingsRow2->addWidget(fileCountLabel);

        separator5 = new QLabel(centralwidget);
        separator5->setObjectName("separator5");

        settingsRow2->addWidget(separator5);

        sortLabel = new QLabel(centralwidget);
        sortLabel->setObjectName("sortLabel");

        settingsRow2->addWidget(sortLabel);

        sortCombo = new QComboBox(centralwidget);
        sortCombo->setObjectName("sortCombo");
        sortCombo->setMaximumSize(QSize(120, 25));

        settingsRow2->addWidget(sortCombo);

        horizontalSpacer4 = new QSpacerItem(0, 0, QSizePolicy::Policy::Expanding, QSizePolicy::Policy::Minimum);

        settingsRow2->addItem(horizontalSpacer4);


        mainLayout->addLayout(settingsRow2);

        progressBar = new QProgressBar(centralwidget);
        progressBar->setObjectName("progressBar");
        progressBar->setVisible(false);

        mainLayout->addWidget(progressBar);

        mainSplitter = new QSplitter(centralwidget);
        mainSplitter->setObjectName("mainSplitter");
        mainSplitter->setOrientation(Qt::Horizontal);
        treeA = new QTreeWidget(mainSplitter);
        treeA->setObjectName("treeA");
        mainSplitter->addWidget(treeA);
        viewerStack = new QStackedWidget(mainSplitter);
        viewerStack->setObjectName("viewerStack");
        mainSplitter->addWidget(viewerStack);
        treeB = new QTreeWidget(mainSplitter);
        treeB->setObjectName("treeB");
        mainSplitter->addWidget(treeB);

        mainLayout->addWidget(mainSplitter);

        DiffLoupe__MainWindow->setCentralWidget(centralwidget);
        menubar = new QMenuBar(DiffLoupe__MainWindow);
        menubar->setObjectName("menubar");
        menubar->setGeometry(QRect(0, 0, 1400, 22));
        DiffLoupe__MainWindow->setMenuBar(menubar);
        statusbar = new QStatusBar(DiffLoupe__MainWindow);
        statusbar->setObjectName("statusbar");
        DiffLoupe__MainWindow->setStatusBar(statusbar);

        retranslateUi(DiffLoupe__MainWindow);

        QMetaObject::connectSlotsByName(DiffLoupe__MainWindow);
    } // setupUi

    void retranslateUi(QMainWindow *DiffLoupe__MainWindow)
    {
        DiffLoupe__MainWindow->setWindowTitle(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\207\343\202\243\343\203\225\343\203\253\343\203\274\343\203\232 - \343\203\225\343\202\251\343\203\253\343\203\200\345\267\256\345\210\206\350\241\250\347\244\272", nullptr));
        selectFolderABtn->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\225\343\202\251\343\203\253\343\203\200A\343\202\222\351\201\270\346\212\236", nullptr));
        folderALabel->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\225\343\202\251\343\203\253\343\203\200A: (\346\234\252\351\201\270\346\212\236)", nullptr));
        selectFolderBBtn->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\225\343\202\251\343\203\253\343\203\200B\343\202\222\351\201\270\346\212\236", nullptr));
        folderBLabel->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\225\343\202\251\343\203\253\343\203\200B: (\346\234\252\351\201\270\346\212\236)", nullptr));
        compareBtn->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\346\257\224\350\274\203 (F5)", nullptr));
        viewModeLabel->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\350\241\250\347\244\272\343\203\242\343\203\274\343\203\211:", nullptr));
        textModeBtn->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\206\343\202\255\343\202\271\343\203\210", nullptr));
        imageModeBtn->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\347\224\273\345\203\217", nullptr));
        hexModeBtn->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\220\343\202\244\343\203\212\343\203\252", nullptr));
        separator1->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "|", nullptr));
        encodingLabel->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\202\250\343\203\263\343\202\263\343\203\274\343\203\207\343\202\243\343\203\263\343\202\260:", nullptr));
        separator2->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "|", nullptr));
        showHiddenCheckbox->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\351\232\240\343\201\227\343\203\225\343\202\241\343\202\244\343\203\253", nullptr));
#if QT_CONFIG(tooltip)
        showHiddenCheckbox->setToolTip(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\211\343\203\203\343\203\210\343\201\247\345\247\213\343\201\276\343\202\213\343\203\225\343\202\241\343\202\244\343\203\253\343\203\273\343\203\225\343\202\251\343\203\253\343\203\200\343\202\222\350\241\250\347\244\272\343\201\227\343\201\276\343\201\231", nullptr));
#endif // QT_CONFIG(tooltip)
        filterLabel->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\225\343\202\243\343\203\253\343\202\277\343\203\274:", nullptr));
        filterAllRadio->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\201\231\343\201\271\343\201\246", nullptr));
#if QT_CONFIG(tooltip)
        filterAllRadio->setToolTip(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\201\231\343\201\271\343\201\246\343\201\256\343\203\225\343\202\241\343\202\244\343\203\253\343\202\222\350\241\250\347\244\272 (Ctrl+1)", nullptr));
#endif // QT_CONFIG(tooltip)
        filterMissingRadio->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\346\254\240\350\220\275\343\203\273\346\226\260\350\246\217", nullptr));
#if QT_CONFIG(tooltip)
        filterMissingRadio->setToolTip(QCoreApplication::translate("DiffLoupe::MainWindow", "\346\254\240\350\220\275\343\201\227\343\201\246\343\201\204\343\202\213\343\203\225\343\202\241\343\202\244\343\203\253\343\202\204\346\226\260\350\246\217\343\203\225\343\202\241\343\202\244\343\203\253\343\201\256\343\201\277\350\241\250\347\244\272 (Ctrl+2)", nullptr));
#endif // QT_CONFIG(tooltip)
        filterDiffRadio->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\345\267\256\345\210\206\343\201\256\343\201\277", nullptr));
#if QT_CONFIG(tooltip)
        filterDiffRadio->setToolTip(QCoreApplication::translate("DiffLoupe::MainWindow", "\345\206\205\345\256\271\343\201\253\345\267\256\345\210\206\343\201\214\343\201\202\343\202\213\343\203\225\343\202\241\343\202\244\343\203\253\343\201\256\343\201\277\350\241\250\347\244\272 (Ctrl+3)", nullptr));
#endif // QT_CONFIG(tooltip)
        separator3->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "|", nullptr));
        filterResetBtn->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\252\343\202\273\343\203\203\343\203\210", nullptr));
#if QT_CONFIG(tooltip)
        filterResetBtn->setToolTip(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\225\343\202\243\343\203\253\343\202\277\343\203\274\343\202\222\343\203\252\343\202\273\343\203\203\343\203\210\343\201\227\343\201\246\343\201\231\343\201\271\343\201\246\350\241\250\347\244\272 (Ctrl+R)", nullptr));
#endif // QT_CONFIG(tooltip)
        separator4->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "|", nullptr));
        fileCountLabel->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\225\343\202\241\343\202\244\343\203\253\346\225\260: 0", nullptr));
        separator5->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "|", nullptr));
        sortLabel->setText(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\202\275\343\203\274\343\203\210:", nullptr));
#if QT_CONFIG(tooltip)
        sortCombo->setToolTip(QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\225\343\202\241\343\202\244\343\203\253\343\201\256\344\270\246\343\201\263\351\240\206\343\202\222\351\201\270\346\212\236", nullptr));
#endif // QT_CONFIG(tooltip)
        QTreeWidgetItem *___qtreewidgetitem = treeA->headerItem();
        ___qtreewidgetitem->setText(1, QCoreApplication::translate("DiffLoupe::MainWindow", "\347\212\266\346\205\213", nullptr));
        ___qtreewidgetitem->setText(0, QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\225\343\202\241\343\202\244\343\203\253", nullptr));
        QTreeWidgetItem *___qtreewidgetitem1 = treeB->headerItem();
        ___qtreewidgetitem1->setText(1, QCoreApplication::translate("DiffLoupe::MainWindow", "\347\212\266\346\205\213", nullptr));
        ___qtreewidgetitem1->setText(0, QCoreApplication::translate("DiffLoupe::MainWindow", "\343\203\225\343\202\241\343\202\244\343\203\253", nullptr));
    } // retranslateUi

};

} // namespace DiffLoupe

namespace DiffLoupe {
namespace Ui {
    class MainWindow: public Ui_MainWindow {};
} // namespace Ui
} // namespace DiffLoupe

#endif // UI_MAINWINDOW_H
