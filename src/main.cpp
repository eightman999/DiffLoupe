//
// Created by eightman on 25/07/13.
//
#include <QApplication>
#include <QTranslator>
#include <QLocale>
#include <QIcon>
#include <QStyleFactory>
#include <QLoggingCategory>
#include <QDebug>
#include "mainwindow.h"

// ログカテゴリの定義
Q_LOGGING_CATEGORY(lcDiffLoupe, "diffloupe")

// Qt警告メッセージの抑制
void setupLogging() {
    // フォント関連の警告を抑制
    QLoggingCategory::setFilterRules(
        "qt.text.font.db.warning=false\n"
        "qt.text.font.db.debug=false\n"
        "qt.qpa.fonts.warning=false\n"
        "qt.qpa.fonts.debug=false"
    );
}

int main(int argc, char *argv[])
{
    // 高DPIサポートの有効化（Qt5/6互換）
#if QT_VERSION < QT_VERSION_CHECK(6, 0, 0)
    QCoreApplication::setAttribute(Qt::AA_EnableHighDpiScaling);
    QCoreApplication::setAttribute(Qt::AA_UseHighDpiPixmaps);
#endif

    QApplication app(argc, argv);

    // アプリケーション情報の設定
    app.setApplicationName("DiffLoupe");
    app.setApplicationDisplayName("ディフルーペ");
    app.setOrganizationName("DiffLoupe");
    app.setOrganizationDomain("diffloupe.local");
    app.setApplicationVersion("1.0.0");

    // 共通アイコン
    app.setWindowIcon(QIcon(":/icons/icon.png"));

    // ログ設定
    setupLogging();

    // スタイルの設定（オプション）
    // 古いOSでも見栄えが良いようにFusionスタイルを使用
    if (QStyleFactory::keys().contains("Fusion")) {
        app.setStyle(QStyleFactory::create("Fusion"));
    }

    // 翻訳ファイルの読み込み（日本語対応）
    QTranslator translator;
    const QStringList uiLanguages = QLocale::system().uiLanguages();
    for (const QString &locale : uiLanguages) {
        const QString baseName = "DiffLoupe_" + QLocale(locale).name();
        if (translator.load(":/i18n/" + baseName)) {
            app.installTranslator(&translator);
            break;
        }
    }

    // メインウィンドウの作成と表示
    DiffLoupe::MainWindow window;

    // アイコンの設定（リソースから）
    window.setWindowIcon(QIcon(":/icons/icon.png"));

    window.show();

    return app.exec();
}
