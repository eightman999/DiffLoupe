# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

DiffLoupe is a cross-platform GUI application for comparing folders and files. It has two main implementations: a C++ version (currently the primary development focus) and a Python/PySide6 version.

## Directory Structure

*   **`/src`**: Contains the C++ source code for the core application.
*   **`/Python`**: Contains the Python/PySide6 source code.
*   **`/forms`**: Contains Qt UI definition files (`.ui`).
*   **`/resources`**: Contains application resources like icons and QRC files.

## Architecture (Python/PySide6 Version)

The application follows a modular structure:

- **main.py**: Entry point that initializes the Qt application and launches the main window
- **ui_main.py**: Contains the main UI components and logic:
  - `MainWindow`: Primary application window with folder selection and comparison controls
  - `DiffViewer`: Side-by-side text diff viewer with line numbers and syntax highlighting
  - `CodeEditor`: Custom text editor with line number display
  - Three view modes: text diff, image preview, and hex viewer
- **comparator.py**: Core comparison logic that efficiently compares folder contents using metadata and hash-based comparison

## Key Features

- **Efficient Comparison**: Uses metadata (size, mtime) for initial comparison, only computing SHA256 hashes when needed
- **Parallel Processing**: Utilizes multiprocessing for hash calculations to improve performance
- **Multiple View Modes**: 
  - Text diff with context and line skipping for large identical sections
  - Enhanced image comparison with 3 modes: side-by-side, toggle, and slice comparison
  - Hex viewer for binary files
- **Advanced Image Comparison**:
  - Side-by-side view for direct visual comparison
  - Toggle view with adjustable timing for animation-like comparison
  - Slice view with slider control for precise overlay comparison
- **Encoding Support**: Extensive character encoding support including Unicode variants, Asian encodings, and legacy formats
- **Hidden Files Control**: Toggle visibility of dot-files and hidden directories
- **Advanced Tree Filtering**:
  - All files mode (default)
  - Missing/New files only mode (files that exist in only one folder)
  - Different files only mode (files with content differences)
- **Flexible Sorting Options**:
  - Sort by name (ascending/descending)
  - Sort by modification time (ascending/descending) 
  - Sort by file size (ascending/descending)
- **Background Processing**: Multithreaded file loading with progress indicators
- **Synchronized Navigation**: Tree views are synchronized between folders for easier comparison
- **Keyboard Shortcuts**: Quick access to filtering modes and comparison functions

## Development Commands

### Running the Application
```bash
# 通常の起動
python main.py

# 警告メッセージを抑制して起動
./run.sh

# または環境変数を設定して起動
export QT_LOGGING_RULES="qt.text.font.db.warning=false"
python main.py
```

### Installing Dependencies
```bash
pip install -r requirements.txt
```

### Keyboard Shortcuts
- **Ctrl+1**: Show all files
- **Ctrl+2**: Show missing/new files only
- **Ctrl+3**: Show different files only
- **Ctrl+R**: Reset filter and sort
- **F5**: Start comparison
- **Ctrl+S**: Focus on sort dropdown
- **Ctrl+Mouse Wheel**: Zoom in/out (in image viewer)

## Dependencies

- **PySide6**: Qt framework for the GUI
- **Pillow**: Image processing and format support
- **chardet**: Character encoding detection

## Code Patterns

- Use Qt's signal-slot pattern for UI interactions
- Custom widgets inherit from Qt base classes and override paint events for custom rendering
- File operations include proper error handling with try/catch blocks
- Japanese comments and UI text indicate the application is designed for Japanese users
- Color coding system for file status (green=match, red=different, gray=missing)

## File Handling

The application handles various file types:
- Text files with encoding detection/selection
- Images (PNG, JPG, TGA, DDS, BMP) with Pillow fallback for unsupported Qt formats
- Binary files displayed in hex format
- Large files are handled efficiently with chunked reading and limited preview sizes


