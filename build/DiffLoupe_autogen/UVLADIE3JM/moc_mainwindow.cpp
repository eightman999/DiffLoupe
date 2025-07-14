/****************************************************************************
** Meta object code from reading C++ file 'mainwindow.h'
**
** Created by: The Qt Meta Object Compiler version 69 (Qt 6.9.0)
**
** WARNING! All changes made in this file will be lost!
*****************************************************************************/

#include "../../../src/mainwindow.h"
#include <QtCore/qmetatype.h>

#include <QtCore/qtmochelpers.h>

#include <memory>


#include <QtCore/qxptype_traits.h>
#if !defined(Q_MOC_OUTPUT_REVISION)
#error "The header file 'mainwindow.h' doesn't include <QObject>."
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
struct qt_meta_tag_ZN9DiffLoupe10MainWindowE_t {};
} // unnamed namespace

template <> constexpr inline auto DiffLoupe::MainWindow::qt_create_metaobjectdata<qt_meta_tag_ZN9DiffLoupe10MainWindowE_t>()
{
    namespace QMC = QtMocConstants;
    QtMocHelpers::StringRefStorage qt_stringData {
        "DiffLoupe::MainWindow",
        "onSelectFolderA",
        "",
        "onSelectFolderB",
        "onCompare",
        "onTreeItemSelected",
        "QTreeWidgetItem*",
        "current",
        "previous",
        "onViewModeChanged",
        "mode",
        "onEncodingChanged",
        "encoding",
        "onFilterChanged",
        "filterId",
        "onSortChanged",
        "index",
        "onShowHiddenChanged",
        "checked",
        "onComparisonFinished",
        "std::vector<DiffResult>",
        "results",
        "onProgressUpdated",
        "message"
    };

    QtMocHelpers::UintData qt_methods {
        // Slot 'onSelectFolderA'
        QtMocHelpers::SlotData<void()>(1, 2, QMC::AccessPrivate, QMetaType::Void),
        // Slot 'onSelectFolderB'
        QtMocHelpers::SlotData<void()>(3, 2, QMC::AccessPrivate, QMetaType::Void),
        // Slot 'onCompare'
        QtMocHelpers::SlotData<void()>(4, 2, QMC::AccessPrivate, QMetaType::Void),
        // Slot 'onTreeItemSelected'
        QtMocHelpers::SlotData<void(QTreeWidgetItem *, QTreeWidgetItem *)>(5, 2, QMC::AccessPrivate, QMetaType::Void, {{
            { 0x80000000 | 6, 7 }, { 0x80000000 | 6, 8 },
        }}),
        // Slot 'onViewModeChanged'
        QtMocHelpers::SlotData<void(int)>(9, 2, QMC::AccessPrivate, QMetaType::Void, {{
            { QMetaType::Int, 10 },
        }}),
        // Slot 'onEncodingChanged'
        QtMocHelpers::SlotData<void(const QString &)>(11, 2, QMC::AccessPrivate, QMetaType::Void, {{
            { QMetaType::QString, 12 },
        }}),
        // Slot 'onFilterChanged'
        QtMocHelpers::SlotData<void(int)>(13, 2, QMC::AccessPrivate, QMetaType::Void, {{
            { QMetaType::Int, 14 },
        }}),
        // Slot 'onSortChanged'
        QtMocHelpers::SlotData<void(int)>(15, 2, QMC::AccessPrivate, QMetaType::Void, {{
            { QMetaType::Int, 16 },
        }}),
        // Slot 'onShowHiddenChanged'
        QtMocHelpers::SlotData<void(bool)>(17, 2, QMC::AccessPrivate, QMetaType::Void, {{
            { QMetaType::Bool, 18 },
        }}),
        // Slot 'onComparisonFinished'
        QtMocHelpers::SlotData<void(const std::vector<DiffResult> &)>(19, 2, QMC::AccessPrivate, QMetaType::Void, {{
            { 0x80000000 | 20, 21 },
        }}),
        // Slot 'onProgressUpdated'
        QtMocHelpers::SlotData<void(const QString &)>(22, 2, QMC::AccessPrivate, QMetaType::Void, {{
            { QMetaType::QString, 23 },
        }}),
    };
    QtMocHelpers::UintData qt_properties {
    };
    QtMocHelpers::UintData qt_enums {
    };
    return QtMocHelpers::metaObjectData<MainWindow, qt_meta_tag_ZN9DiffLoupe10MainWindowE_t>(QMC::MetaObjectFlag{}, qt_stringData,
            qt_methods, qt_properties, qt_enums);
}
Q_CONSTINIT const QMetaObject DiffLoupe::MainWindow::staticMetaObject = { {
    QMetaObject::SuperData::link<QMainWindow::staticMetaObject>(),
    qt_staticMetaObjectStaticContent<qt_meta_tag_ZN9DiffLoupe10MainWindowE_t>.stringdata,
    qt_staticMetaObjectStaticContent<qt_meta_tag_ZN9DiffLoupe10MainWindowE_t>.data,
    qt_static_metacall,
    nullptr,
    qt_staticMetaObjectRelocatingContent<qt_meta_tag_ZN9DiffLoupe10MainWindowE_t>.metaTypes,
    nullptr
} };

void DiffLoupe::MainWindow::qt_static_metacall(QObject *_o, QMetaObject::Call _c, int _id, void **_a)
{
    auto *_t = static_cast<MainWindow *>(_o);
    if (_c == QMetaObject::InvokeMetaMethod) {
        switch (_id) {
        case 0: _t->onSelectFolderA(); break;
        case 1: _t->onSelectFolderB(); break;
        case 2: _t->onCompare(); break;
        case 3: _t->onTreeItemSelected((*reinterpret_cast< std::add_pointer_t<QTreeWidgetItem*>>(_a[1])),(*reinterpret_cast< std::add_pointer_t<QTreeWidgetItem*>>(_a[2]))); break;
        case 4: _t->onViewModeChanged((*reinterpret_cast< std::add_pointer_t<int>>(_a[1]))); break;
        case 5: _t->onEncodingChanged((*reinterpret_cast< std::add_pointer_t<QString>>(_a[1]))); break;
        case 6: _t->onFilterChanged((*reinterpret_cast< std::add_pointer_t<int>>(_a[1]))); break;
        case 7: _t->onSortChanged((*reinterpret_cast< std::add_pointer_t<int>>(_a[1]))); break;
        case 8: _t->onShowHiddenChanged((*reinterpret_cast< std::add_pointer_t<bool>>(_a[1]))); break;
        case 9: _t->onComparisonFinished((*reinterpret_cast< std::add_pointer_t<std::vector<DiffResult>>>(_a[1]))); break;
        case 10: _t->onProgressUpdated((*reinterpret_cast< std::add_pointer_t<QString>>(_a[1]))); break;
        default: ;
        }
    }
}

const QMetaObject *DiffLoupe::MainWindow::metaObject() const
{
    return QObject::d_ptr->metaObject ? QObject::d_ptr->dynamicMetaObject() : &staticMetaObject;
}

void *DiffLoupe::MainWindow::qt_metacast(const char *_clname)
{
    if (!_clname) return nullptr;
    if (!strcmp(_clname, qt_staticMetaObjectStaticContent<qt_meta_tag_ZN9DiffLoupe10MainWindowE_t>.strings))
        return static_cast<void*>(this);
    return QMainWindow::qt_metacast(_clname);
}

int DiffLoupe::MainWindow::qt_metacall(QMetaObject::Call _c, int _id, void **_a)
{
    _id = QMainWindow::qt_metacall(_c, _id, _a);
    if (_id < 0)
        return _id;
    if (_c == QMetaObject::InvokeMetaMethod) {
        if (_id < 11)
            qt_static_metacall(this, _c, _id, _a);
        _id -= 11;
    }
    if (_c == QMetaObject::RegisterMethodArgumentMetaType) {
        if (_id < 11)
            *reinterpret_cast<QMetaType *>(_a[0]) = QMetaType();
        _id -= 11;
    }
    return _id;
}
QT_WARNING_POP
