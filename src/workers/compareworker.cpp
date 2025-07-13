#include "workers/compareworker.h"
#include "core/comparator.h"

namespace DiffLoupe {

    CompareWorker::CompareWorker(QObject *parent)
        : QThread(parent)
    {
    }

    CompareWorker::~CompareWorker()
    {
        if (isRunning()) {
            quit();
            wait();
        }
    }

    void CompareWorker::setFolders(const QString &folderA, const QString &folderB)
    {
        m_folderA = folderA;
        m_folderB = folderB;
    }

    void CompareWorker::setSettings(const Comparator::Settings &settings)
    {
        m_settings = settings;
    }

    void CompareWorker::run()
    {
        try {
            emit progressUpdated("比較を開始しています...");

            Comparator comparator;

            // 進捗コールバックを設定
            auto progressCallback = [this](const QString &message, int percentage) {
                emit progressUpdated(message);
                emit progressPercentage(percentage);
            };

            // Comparatorからのシグナルを転送
            connect(&comparator, &Comparator::progressUpdated,
                    this, &CompareWorker::progressUpdated);

            // 比較を実行
            QStringList folders = { m_folderA, m_folderB };
            auto results = comparator.compareAll(folders, m_settings, progressCallback);

            emit comparisonFinished(results);
            emit progressUpdated("比較が完了しました");

        } catch (const std::exception &e) {
            emit errorOccurred(QString("エラーが発生しました: %1").arg(e.what()));
        } catch (...) {
            emit errorOccurred("不明なエラーが発生しました");
        }
    }

} // namespace DiffLoupe