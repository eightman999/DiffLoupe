// Copyright (c) eightman 2005-2025
// Furin-lab All rights reserved.
// 動作設計: ImageViewerクラス宣言。画像比較ビューのUI定義

#ifndef IMAGEVIEWER_H
#define IMAGEVIEWER_H

#include <QWidget>
#include <QPixmap>
#include <QTimer>
#include <QPointF>
#include <QSlider>
#include <QLabel>

class QRadioButton;
class QGroupBox;

namespace DiffLoupe {

class ImageViewer : public QWidget {
    Q_OBJECT

public:
    explicit ImageViewer(QWidget *parent = nullptr);
    ~ImageViewer();

    void setImages(const QString &imageA, const QString &imageB);
    void clear();

protected:
    void paintEvent(QPaintEvent *event) override;
    void wheelEvent(QWheelEvent *event) override;
    void mousePressEvent(QMouseEvent *event) override;
    void mouseMoveEvent(QMouseEvent *event) override;
    void mouseReleaseEvent(QMouseEvent *event) override;

private slots:
    void updateViewMode();
    void toggleImage();
    void updateDiffThreshold(int threshold);

private:
    // 比較モード
    enum class ViewMode {
        SideBySide,
        Toggle,
        DiffHighlight
    };

    void setupUI();
    void resetView();
    void recalculateDifference();
    int colorDifference(const QColor& c1, const QColor& c2);
    QImage generateDiffImage(const QImage& imageA, const QImage& imageB, int threshold);

    // UI Elements
    QRadioButton *m_sideBySideRadio;
    QRadioButton *m_toggleRadio;
    QRadioButton *m_diffHighlightRadio;
    QGroupBox *m_modeGroup;
    QSlider *m_sensitivitySlider;
    QLabel *m_statsLabel;
    QLabel *m_sensitivityLabel;

    // Image data
    QPixmap m_pixmapA;
    QPixmap m_pixmapB;
    QPixmap m_diffPixmap;

    // View state
    ViewMode m_currentMode = ViewMode::SideBySide;
    qreal m_scale = 1.0;
    QPointF m_panOffset;
    QPoint m_lastPanPoint;
    bool m_isPanning = false;

    // Toggle mode
    QTimer *m_toggleTimer;
    bool m_isShowingImageA = true;

    // Diff mode
    int m_diffThreshold = 30;
    long m_diffPixelCount = 0;
    double m_diffPercentage = 0.0;
};

} // namespace DiffLoupe

#endif // IMAGEVIEWER_H
