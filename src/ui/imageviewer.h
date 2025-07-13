#ifndef IMAGEVIEWER_H
#define IMAGEVIEWER_H

#include <QWidget>
#include <QLabel>
#include <QVBoxLayout>
#include <QPixmap>
#include <QScrollArea>

namespace DiffLoupe {

class ImageViewer : public QWidget {
    Q_OBJECT

public:
    explicit ImageViewer(QWidget *parent = nullptr);
    ~ImageViewer();

    void setImages(const QString &imageA, const QString &imageB);
    void clear();

private:
    QLabel *m_imageLabelA;
    QLabel *m_imageLabelB;
    QScrollArea *m_scrollAreaA;
    QScrollArea *m_scrollAreaB;
};

} // namespace DiffLoupe

#endif // IMAGEVIEWER_H
