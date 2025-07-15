#include "imageviewer.h"
#include <QPainter>
#include <QWheelEvent>
#include <QMouseEvent>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QRadioButton>
#include <QGroupBox>
#include <QLabel>

namespace DiffLoupe {

ImageViewer::ImageViewer(QWidget *parent)
    : QWidget(parent),
      m_toggleTimer(new QTimer(this))
{
    setupUI();
    connect(m_toggleTimer, &QTimer::timeout, this, &ImageViewer::toggleImage);
    m_toggleTimer->setInterval(500); // 0.5秒間隔
}

ImageViewer::~ImageViewer() = default;

void ImageViewer::setupUI() {
    QVBoxLayout *mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(5, 5, 5, 5);
    
    // View mode controls
    m_modeGroup = new QGroupBox("View Mode");
    QHBoxLayout *modeLayout = new QHBoxLayout;
    
    m_sideBySideRadio = new QRadioButton("Side-by-Side");
    m_sideBySideRadio->setChecked(true);
    connect(m_sideBySideRadio, &QRadioButton::toggled, this, &ImageViewer::updateViewMode);
    
    m_toggleRadio = new QRadioButton("Toggle");
    connect(m_toggleRadio, &QRadioButton::toggled, this, &ImageViewer::updateViewMode);

    m_diffHighlightRadio = new QRadioButton("Diff Highlight");
    connect(m_diffHighlightRadio, &QRadioButton::toggled, this, &ImageViewer::updateViewMode);

    modeLayout->addWidget(m_sideBySideRadio);
    modeLayout->addWidget(m_toggleRadio);
    modeLayout->addWidget(m_diffHighlightRadio);
    modeLayout->addStretch();
    m_modeGroup->setLayout(modeLayout);

    // 差分感度調整スライダー
    m_sensitivityLabel = new QLabel("差分感度:");
    m_sensitivitySlider = new QSlider(Qt::Horizontal);
    m_sensitivitySlider->setRange(0, 255);
    m_sensitivitySlider->setValue(m_diffThreshold);
    connect(m_sensitivitySlider, &QSlider::valueChanged, this, &ImageViewer::updateDiffThreshold);

    QHBoxLayout *sensitivityLayout = new QHBoxLayout;
    sensitivityLayout->addWidget(m_sensitivityLabel);
    sensitivityLayout->addWidget(m_sensitivitySlider);
    sensitivityLayout->addStretch();

    // 差分統計情報
    m_statsLabel = new QLabel("差分統計: --");
    m_statsLabel->setStyleSheet("color: gray; font-size: 10px;");

    // 使用説明
    QLabel *helpLabel = new QLabel("Ctrl+マウスホイール: ズーム, マウスドラッグ: パン");
    helpLabel->setStyleSheet("color: gray; font-size: 10px;");

    mainLayout->addWidget(m_modeGroup);
    mainLayout->addLayout(sensitivityLayout);
    mainLayout->addWidget(m_statsLabel);
    mainLayout->addWidget(helpLabel);
    mainLayout->addStretch(); // 画像描画エリアを残す

    setLayout(mainLayout);
    setMinimumSize(400, 300);
}

void ImageViewer::setImages(const QString &imageA, const QString &imageB) {
    if (!imageA.isEmpty()) {
        m_pixmapA.load(imageA);
    } else {
        m_pixmapA = QPixmap();
    }
    
    if (!imageB.isEmpty()) {
        m_pixmapB.load(imageB);
    } else {
        m_pixmapB = QPixmap();
    }
    
    resetView();
    update(); // 再描画をスケジュール
}

void ImageViewer::clear() {
    m_pixmapA = QPixmap();
    m_pixmapB = QPixmap();
    resetView();
    update();
}

void ImageViewer::resetView() {
    m_scale = 1.0;
    m_panOffset = QPointF(0, 0);
    m_isShowingImageA = true;
}

void ImageViewer::paintEvent(QPaintEvent *event) {
    Q_UNUSED(event);
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);

    // 背景をグレーで塗りつぶし
    painter.fillRect(rect(), QColor(220, 220, 220));

    // コントロールエリアの高さを考慮して描画エリアを調整
    QRect drawArea = rect();
    if (m_modeGroup) {
        drawArea.setTop(m_modeGroup->geometry().bottom() + 30); // コントロール部分を除く
    }

    // 描画エリアの中央を計算
    QPointF center = drawArea.center();
    
    // 変換を適用
    painter.translate(center + m_panOffset);
    painter.scale(m_scale, m_scale);

    if (m_currentMode == ViewMode::SideBySide) {
        if (!m_pixmapA.isNull() && !m_pixmapB.isNull()) {
            int totalWidth = m_pixmapA.width() + m_pixmapB.width();
            int maxHeight = qMax(m_pixmapA.height(), m_pixmapB.height());
            
            // 画像Aを左側に描画
            painter.drawPixmap(-totalWidth / 2, -maxHeight / 2, m_pixmapA);
            
            // 画像Bを右側に描画
            painter.drawPixmap(-totalWidth / 2 + m_pixmapA.width(), -maxHeight / 2, m_pixmapB);
        } else if (!m_pixmapA.isNull()) {
            painter.drawPixmap(-m_pixmapA.width() / 2, -m_pixmapA.height() / 2, m_pixmapA);
        } else if (!m_pixmapB.isNull()) {
            painter.drawPixmap(-m_pixmapB.width() / 2, -m_pixmapB.height() / 2, m_pixmapB);
        }
    } else if (m_currentMode == ViewMode::Toggle) {
        const QPixmap &currentPixmap = m_isShowingImageA ? m_pixmapA : m_pixmapB;
        if (!currentPixmap.isNull()) {
            painter.drawPixmap(-currentPixmap.width() / 2, -currentPixmap.height() / 2, currentPixmap);
        }
    } else if (m_currentMode == ViewMode::DiffHighlight) {
        // 差分ハイライト表示
        if (!m_pixmapA.isNull()) {
            painter.drawPixmap(-m_pixmapA.width() / 2, -m_pixmapA.height() / 2, m_pixmapA);
        }
        if (!m_diffPixmap.isNull()) {
            painter.drawPixmap(-m_diffPixmap.width() / 2, -m_diffPixmap.height() / 2, m_diffPixmap);
        }
    }
}

void ImageViewer::wheelEvent(QWheelEvent *event) {
    if (event->modifiers() & Qt::ControlModifier) {
        qreal scaleFactor = 1.15;
        if (event->angleDelta().y() > 0) {
            m_scale *= scaleFactor; // ズームイン
        } else {
            m_scale /= scaleFactor; // ズームアウト
        }
        
        // スケールを制限
        m_scale = qMax(0.1, qMin(m_scale, 10.0));
        
        update();
        event->accept();
    } else {
        event->ignore();
    }
}

void ImageViewer::mousePressEvent(QMouseEvent *event) {
    if (event->button() == Qt::LeftButton) {
        m_isPanning = true;
        m_lastPanPoint = event->pos();
        setCursor(Qt::ClosedHandCursor);
        event->accept();
    } else {
        event->ignore();
    }
}

void ImageViewer::mouseMoveEvent(QMouseEvent *event) {
    if (m_isPanning) {
        QPoint delta = event->pos() - m_lastPanPoint;
        m_panOffset += delta;
        m_lastPanPoint = event->pos();
        update();
        event->accept();
    } else {
        event->ignore();
    }
}

void ImageViewer::mouseReleaseEvent(QMouseEvent *event) {
    if (event->button() == Qt::LeftButton && m_isPanning) {
        m_isPanning = false;
        setCursor(Qt::ArrowCursor);
        event->accept();
    } else {
        event->ignore();
    }
}

void ImageViewer::updateViewMode() {
    if (m_sideBySideRadio->isChecked()) {
        m_currentMode = ViewMode::SideBySide;
        m_toggleTimer->stop();
    } else if (m_toggleRadio->isChecked()) {
        m_currentMode = ViewMode::Toggle;
        m_toggleTimer->start();
    } else if (m_diffHighlightRadio->isChecked()) {
        m_currentMode = ViewMode::DiffHighlight;
        m_toggleTimer->stop();
        recalculateDifference();
    }
    update();
}

void ImageViewer::toggleImage() {
    if (m_currentMode == ViewMode::Toggle) {
        m_isShowingImageA = !m_isShowingImageA;
        update();
    }
}

void ImageViewer::updateDiffThreshold(int threshold) {
    m_diffThreshold = threshold;
    if (m_currentMode == ViewMode::DiffHighlight) {
        recalculateDifference();
    }
}

void ImageViewer::recalculateDifference() {
    if (m_pixmapA.isNull() || m_pixmapB.isNull()) {
        m_diffPixmap = QPixmap();
        m_diffPixelCount = 0;
        m_diffPercentage = 0.0;
        m_statsLabel->setText("差分統計: --");
        return;
    }

    // QPixmapをQImageに変換
    QImage imageA = m_pixmapA.toImage();
    QImage imageB = m_pixmapB.toImage();

    // 差分画像を生成
    QImage diffImage = generateDiffImage(imageA, imageB, m_diffThreshold);
    m_diffPixmap = QPixmap::fromImage(diffImage);

    // 統計情報を更新
    QString stats = QString("差分統計: %1 pixels (%2%)")
                        .arg(m_diffPixelCount)
                        .arg(m_diffPercentage, 0, 'f', 2);
    m_statsLabel->setText(stats);

    update();
}

int ImageViewer::colorDifference(const QColor& c1, const QColor& c2) {
    return std::abs(c1.red() - c2.red()) +
           std::abs(c1.green() - c2.green()) +
           std::abs(c1.blue() - c2.blue());
}

QImage ImageViewer::generateDiffImage(const QImage& imageA, const QImage& imageB, int threshold) {
    // 両方の画像の最大サイズを取得
    int maxWidth = std::max(imageA.width(), imageB.width());
    int maxHeight = std::max(imageA.height(), imageB.height());

    // 差分画像を作成（透明背景）
    QImage diffImage(maxWidth, maxHeight, QImage::Format_ARGB32_Premultiplied);
    diffImage.fill(Qt::transparent);

    m_diffPixelCount = 0;
    long totalPixels = maxWidth * maxHeight;

    for (int y = 0; y < maxHeight; ++y) {
        for (int x = 0; x < maxWidth; ++x) {
            // 各画像のピクセル色を取得（範囲外の場合は透明）
            QColor colorA = Qt::transparent;
            QColor colorB = Qt::transparent;

            if (x < imageA.width() && y < imageA.height()) {
                colorA = imageA.pixelColor(x, y);
            }
            if (x < imageB.width() && y < imageB.height()) {
                colorB = imageB.pixelColor(x, y);
            }

            // 差分を計算
            int diff = colorDifference(colorA, colorB);

            if (diff > threshold) {
                // 差分があるピクセルを赤色でハイライト
                diffImage.setPixelColor(x, y, QColor(255, 0, 0, 150)); // 半透明の赤
                m_diffPixelCount++;
            }
        }
    }

    // 差分パーセンテージを計算
    m_diffPercentage = (static_cast<double>(m_diffPixelCount) / totalPixels) * 100.0;

    return diffImage;
}

} // namespace DiffLoupe