cmake_minimum_required(VERSION 3.16)
project(DiffLoupe VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

set(CMAKE_AUTOUIC_SEARCH_PATHS ${CMAKE_CURRENT_SOURCE_DIR}/forms)

# Qt5とQt6の両方に対応
find_package(QT NAMES Qt6 Qt5 REQUIRED COMPONENTS Core)

if (QT_VERSION_MAJOR EQUAL 6)
    # Vulkanヘッダーが無い環境でも警告のみで進むようにする
    set(CMAKE_DISABLE_FIND_PACKAGE_WrapVulkanHeaders ON)
    find_package(Qt6 REQUIRED COMPONENTS Core Widgets Concurrent Core5Compat)
else()
    find_package(Qt5 REQUIRED COMPONENTS Core Widgets Concurrent)
endif()

# ソースファイル
set(SOURCES
        src/main.cpp
        src/mainwindow.cpp
        src/core/comparator.cpp
        src/core/fileinfo.cpp
        src/core/diffresult.cpp
        src/workers/compareworker.cpp
        src/workers/fileloadworker.cpp
        src/ui/diffviewer.cpp
        src/ui/imageviewer.cpp
        src/ui/hexviewer.cpp
)

# ヘッダーファイル
set(HEADERS
        src/mainwindow.h
        src/core/comparator.h
        src/core/fileinfo.h
        src/core/diffresult.h
        src/ui/diffviewer.h
        src/ui/imageviewer.h
        src/ui/hexviewer.h
        src/workers/compareworker.h
        src/workers/fileloadworker.h
)

# UIファイル
set(UI_FILES
        forms/mainwindow.ui
)

# リソースファイル
set(RESOURCE_FILES
        resources/resources.qrc
)

# 実行ファイルの作成
if(${QT_VERSION_MAJOR} GREATER_EQUAL 6)
    qt_add_executable(DiffLoupe
            MANUAL_FINALIZATION
            ${SOURCES}
            ${HEADERS}
            ${UI_FILES}
            ${RESOURCE_FILES}
    )
else()
    add_executable(DiffLoupe
            ${SOURCES}
            ${HEADERS}
            ${UI_FILES}
            ${RESOURCE_FILES}
    )
endif()

# ライブラリのリンク
target_link_libraries(DiffLoupe PRIVATE
        Qt${QT_VERSION_MAJOR}::Core
        Qt${QT_VERSION_MAJOR}::Widgets
        Qt${QT_VERSION_MAJOR}::Concurrent
        $<$<EQUAL:${QT_VERSION_MAJOR},6>:Qt6::Core5Compat>
)

# インクルードディレクトリの設定
target_include_directories(DiffLoupe PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/src
        ${CMAKE_CURRENT_SOURCE_DIR}/forms
        ${CMAKE_CURRENT_BINARY_DIR}
)

# Windows向けの設定
if(WIN32)
    set_target_properties(DiffLoupe PROPERTIES
            WIN32_EXECUTABLE TRUE
    )
endif()

# macOS向けの設定
if(APPLE)
    set_target_properties(DiffLoupe PROPERTIES
            MACOSX_BUNDLE TRUE
            MACOSX_BUNDLE_GUI_IDENTIFIER "com.diffloupe.DiffLoupe"
            MACOSX_BUNDLE_BUNDLE_VERSION ${PROJECT_VERSION}
            MACOSX_BUNDLE_SHORT_VERSION_STRING ${PROJECT_VERSION}
            MACOSX_BUNDLE_INFO_STRING "DiffLoupe - Folder Diff Viewer"
            MACOSX_BUNDLE_BUNDLE_NAME "DiffLoupe"
            MACOSX_BUNDLE_COPYRIGHT "Copyright © 2025 DiffLoupe"
    )

    # Info.plist.inが存在する場合のみ使用
    if(EXISTS ${CMAKE_CURRENT_SOURCE_DIR}/Info.plist.in)
        set_target_properties(DiffLoupe PROPERTIES
                MACOSX_BUNDLE_INFO_PLIST ${CMAKE_CURRENT_SOURCE_DIR}/Info.plist.in
        )
    endif()
endif()

# Qt6の場合の最終処理
if(QT_VERSION_MAJOR EQUAL 6)
    qt_finalize_executable(DiffLoupe)
endif()

# インストール設定
install(TARGETS DiffLoupe
        BUNDLE DESTINATION .
        LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
        RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
)