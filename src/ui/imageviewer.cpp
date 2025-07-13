#include "imageviewer.h"
#include <QHBoxLayout>

namespace DiffLoupe {

ImageViewer::ImageViewer(QWidget *parent) : QWidget(parent) {
    m_imageLabelA = new QLabel(this);
    m_imageLabelA->setAlignment(Qt::AlignCenter);
    m_imageLabelA->setScaledContents(true);
    m_scrollAreaA = new QScrollArea(this);
    m_scrollAreaA->setWidget(m_imageLabelA);
    m_scrollAreaA->setWidgetResizable(true);

    m_imageLabelB = new QLabel(this);
    m_imageLabelB->setAlignment(Qt::AlignCenter);
    m_imageLabelB->setScaledContents(true);
    m_scrollAreaB = new QScrollArea(this);
    m_scrollAreaB->setWidget(m_imageLabelB);
    m_scrollAreaB->setWidgetResizable(true);

    QHBoxLayout *layout = new QHBoxLayout(this);
    layout->addWidget(m_scrollAreaA);
    layout->addWidget(m_scrollAreaB);
    setLayout(layout);
}

ImageViewer::~ImageViewer() = default;

void ImageViewer::setImages(const QString &imageA, const QString &imageB) {
    QPixmap pixmapA;
    if (!imageA.isEmpty()) {
        pixmapA.load(imageA);
    }
    QPixmap pixmapB;
    if (!imageB.isEmpty()) {
        pixmapB.load(imageB);
    }

    m_imageLabelA->setPixmap(pixmapA);
    m_imageLabelB->setPixmap(pixmapB);

    m_imageLabelA->adjustSize();
    m_imageLabelB->adjustSize();
}

void ImageViewer::clear() {
    m_imageLabelA->clear();
    m_imageLabelB->clear();
}

} // namespace DiffLoupe