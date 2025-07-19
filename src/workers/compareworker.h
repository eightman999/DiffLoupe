// Copyright (c) eightman 2005-2025
// Furin-lab All rights reserved.
// 動作設計: 比較処理を実行するバックグラウンドスレッドクラスの宣言

//
// Created by eightman on 25/07/13.
//

#ifndef COMPAREWORKER_H
#define COMPAREWORKER_H

#include <QThread>
#include <QString>
#include <vector>
#include "core/diffresult.h"
#include "core/comparator.h"

namespace DiffLoupe {

    class CompareWorker : public QThread {
        Q_OBJECT

    public:
        explicit CompareWorker(QObject *parent = nullptr);
        ~CompareWorker();

        // 比較パラメータ設定
        void setFolders(const QString &folderA, const QString &folderB);
        void setSettings(const Comparator::Settings &settings);

        signals:
            // 比較完了
            void comparisonFinished(const std::vector<DiffResult> &results);

        // 進捗更新
        void progressUpdated(const QString &message);
        void progressPercentage(int percentage);

        // エラー発生
        void errorOccurred(const QString &error);

    protected:
        void run() override;

    private:
        QString m_folderA;
        QString m_folderB;
        Comparator::Settings m_settings;
    };

} // namespace DiffLoupe

#endif // COMPAREWORKER_H
