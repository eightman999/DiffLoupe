/****************************************************************************
** Meta object code from reading C++ file 'compareworker.h'
**
** Created by: The Qt Meta Object Compiler version 69 (Qt 6.9.0)
**
** WARNING! All changes made in this file will be lost!
*****************************************************************************/

#include "../../../src/workers/compareworker.h"
#include <QtCore/qmetatype.h>

#include <QtCore/qtmochelpers.h>

#include <memory>


#include <QtCore/qxptype_traits.h>
#if !defined(Q_MOC_OUTPUT_REVISION)
#error "The header file 'compareworker.h' doesn't include <QObject>."
#elif Q_MOC_OUTPUT_REVISION != 69
#error "This file was generated using the moc from 6.9.0. It"
#error "cannot be used with the include files from this version of Qt."
#error "(The moc has changed too much.)"
#endif

#ifndef Q_CONSTINIT
#define Q_CONSTINIT
#endif

QT_WARNING_PUSH
QT_WARNING_DISABLE_DEPRECATED
QT_WARNING_DISABLE_GCC("-Wuseless-cast")
namespace {
struct qt_meta_tag_ZN9DiffLoupe13CompareWorkerE_t {};
} // unnamed namespace

template <> constexpr inline auto DiffLoupe::CompareWorker::qt_create_metaobjectdata<qt_meta_tag_ZN9DiffLoupe13CompareWorkerE_t>()
{
    namespace QMC = QtMocConstants;
    QtMocHelpers::StringRefStorage qt_stringData {
        "DiffLoupe::CompareWorker",
        "comparisonFinished",
        "",
        "std::vector<DiffResult>",
        "results",
        "progressUpdated",
        "message",
        "progressPercentage",
        "percentage",
        "errorOccurred",
        "error"
    };

    QtMocHelpers::UintData qt_methods {
        // Signal 'comparisonFinished'
        QtMocHelpers::SignalData<void(const std::vector<DiffResult> &)>(1, 2, QMC::AccessPublic, QMetaType::Void, {{
            { 0x80000000 | 3, 4 },
        }}),
        // Signal 'progressUpdated'
        QtMocHelpers::SignalData<void(const QString &)>(5, 2, QMC::AccessPublic, QMetaType::Void, {{
            { QMetaType::QString, 6 },
        }}),
        // Signal 'progressPercentage'
        QtMocHelpers::SignalData<void(int)>(7, 2, QMC::AccessPublic, QMetaType::Void, {{
            { QMetaType::Int, 8 },
        }}),
        // Signal 'errorOccurred'
        QtMocHelpers::SignalData<void(const QString &)>(9, 2, QMC::AccessPublic, QMetaType::Void, {{
            { QMetaType::QString, 10 },
        }}),
    };
    QtMocHelpers::UintData qt_properties {
    };
    QtMocHelpers::UintData qt_enums {
    };
    return QtMocHelpers::metaObjectData<CompareWorker, qt_meta_tag_ZN9DiffLoupe13CompareWorkerE_t>(QMC::MetaObjectFlag{}, qt_stringData,
            qt_methods, qt_properties, qt_enums);
}
Q_CONSTINIT const QMetaObject DiffLoupe::CompareWorker::staticMetaObject = { {
    QMetaObject::SuperData::link<QThread::staticMetaObject>(),
    qt_staticMetaObjectStaticContent<qt_meta_tag_ZN9DiffLoupe13CompareWorkerE_t>.stringdata,
    qt_staticMetaObjectStaticContent<qt_meta_tag_ZN9DiffLoupe13CompareWorkerE_t>.data,
    qt_static_metacall,
    nullptr,
    qt_staticMetaObjectRelocatingContent<qt_meta_tag_ZN9DiffLoupe13CompareWorkerE_t>.metaTypes,
    nullptr
} };

void DiffLoupe::CompareWorker::qt_static_metacall(QObject *_o, QMetaObject::Call _c, int _id, void **_a)
{
    auto *_t = static_cast<CompareWorker *>(_o);
    if (_c == QMetaObject::InvokeMetaMethod) {
        switch (_id) {
        case 0: _t->comparisonFinished((*reinterpret_cast< std::add_pointer_t<std::vector<DiffResult>>>(_a[1]))); break;
        case 1: _t->progressUpdated((*reinterpret_cast< std::add_pointer_t<QString>>(_a[1]))); break;
        case 2: _t->progressPercentage((*reinterpret_cast< std::add_pointer_t<int>>(_a[1]))); break;
        case 3: _t->errorOccurred((*reinterpret_cast< std::add_pointer_t<QString>>(_a[1]))); break;
        default: ;
        }
    }
    if (_c == QMetaObject::IndexOfMethod) {
        if (QtMocHelpers::indexOfMethod<void (CompareWorker::*)(const std::vector<DiffResult> & )>(_a, &CompareWorker::comparisonFinished, 0))
            return;
        if (QtMocHelpers::indexOfMethod<void (CompareWorker::*)(const QString & )>(_a, &CompareWorker::progressUpdated, 1))
            return;
        if (QtMocHelpers::indexOfMethod<void (CompareWorker::*)(int )>(_a, &CompareWorker::progressPercentage, 2))
            return;
        if (QtMocHelpers::indexOfMethod<void (CompareWorker::*)(const QString & )>(_a, &CompareWorker::errorOccurred, 3))
            return;
    }
}

const QMetaObject *DiffLoupe::CompareWorker::metaObject() const
{
    return QObject::d_ptr->metaObject ? QObject::d_ptr->dynamicMetaObject() : &staticMetaObject;
}

void *DiffLoupe::CompareWorker::qt_metacast(const char *_clname)
{
    if (!_clname) return nullptr;
    if (!strcmp(_clname, qt_staticMetaObjectStaticContent<qt_meta_tag_ZN9DiffLoupe13CompareWorkerE_t>.strings))
        return static_cast<void*>(this);
    return QThread::qt_metacast(_clname);
}

int DiffLoupe::CompareWorker::qt_metacall(QMetaObject::Call _c, int _id, void **_a)
{
    _id = QThread::qt_metacall(_c, _id, _a);
    if (_id < 0)
        return _id;
    if (_c == QMetaObject::InvokeMetaMethod) {
        if (_id < 4)
            qt_static_metacall(this, _c, _id, _a);
        _id -= 4;
    }
    if (_c == QMetaObject::RegisterMethodArgumentMetaType) {
        if (_id < 4)
            *reinterpret_cast<QMetaType *>(_a[0]) = QMetaType();
        _id -= 4;
    }
    return _id;
}

// SIGNAL 0
void DiffLoupe::CompareWorker::comparisonFinished(const std::vector<DiffResult> & _t1)
{
    QMetaObject::activate<void>(this, &staticMetaObject, 0, nullptr, _t1);
}

// SIGNAL 1
void DiffLoupe::CompareWorker::progressUpdated(const QString & _t1)
{
    QMetaObject::activate<void>(this, &staticMetaObject, 1, nullptr, _t1);
}

// SIGNAL 2
void DiffLoupe::CompareWorker::progressPercentage(int _t1)
{
    QMetaObject::activate<void>(this, &staticMetaObject, 2, nullptr, _t1);
}

// SIGNAL 3
void DiffLoupe::CompareWorker::errorOccurred(const QString & _t1)
{
    QMetaObject::activate<void>(this, &staticMetaObject, 3, nullptr, _t1);
}
QT_WARNING_POP
