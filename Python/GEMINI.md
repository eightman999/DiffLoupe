# GEMINI.md

This file provides guidance to Gemini when working with code in this repository.

## Project Overview

DiffLoupe is a cross-platform GUI application for comparing folders and files. It has two main implementations: a C++ version (currently the primary development focus) and a Python/PySide6 version.

## Directory Structure

*   **`/src`**: Contains the C++ source code for the core application.
*   **`/Python`**: Contains the Python/PySide6 source code.
*   **`/forms`**: Contains Qt UI definition files (`.ui`).
*   **`/resources`**: Contains application resources like icons and QRC files.

## Architecture (Python/PySide6 Version)

The application is structured as follows:

- **main.py**: The application's entry point. It initializes the Qt application and the main window.
- **ui_main.py**: This file contains the core UI logic and components:
  - `MainWindow`: The main application window, which handles folder selection, comparison controls, and filtering.
  - `DiffViewer`: A side-by-side text diff viewer with features like line numbers and syntax highlighting.
  - `CodeEditor`: A custom text editor widget that includes a line number display.
  - The application supports three main view modes: text diff, image preview, and a hex viewer for binary files.
- **comparator.py**: This module contains the core logic for comparing files and folders. It is designed to be efficient, using file metadata for initial checks and falling back to hash-based comparison when necessary.

## Key Features

- **Efficient Comparison**: The application uses file metadata (size and modification time) for quick comparisons, and only computes SHA256 hashes for files that appear to be different.
- **Parallel Processing**: To improve performance, the application uses multiprocessing for hash calculations.
- **Multiple View Modes**:
  - A text diff view with context and line skipping for large files.
  - An enhanced image comparison view with three modes: side-by-side, toggle, and a slice view for detailed comparison.
  - A hex viewer for binary files.
- **Advanced Image Comparison**:
  - **Side-by-side view**: For direct visual comparison.
  - **Toggle view**: Allows for an animated comparison by toggling between images with an adjustable timer.
  - **Slice view**: Provides a slider to control an overlay for precise comparison.
- **Encoding Support**: The application supports a wide range of character encodings, including Unicode, Asian encodings, and various legacy formats.
- **Hidden Files**: Users can toggle the visibility of hidden files and directories.
- **Advanced Filtering**:
  - **All files mode**: The default view.
  - **Missing/New files only**: Shows only the files that exist in one of the two selected folders.
  - **Different files only**: Shows only the files that have content differences.
- **Flexible Sorting**: Files can be sorted by name, modification time, or size, in either ascending or descending order.
- **Background Processing**: File loading is handled in a separate thread to keep the UI responsive, with progress indicators to show the status.
- **Synchronized Navigation**: The tree views for the two selected folders are synchronized for easier comparison.
- **Keyboard Shortcuts**: The application provides keyboard shortcuts for common actions like filtering, comparison, and zooming.

## Development Commands

### Running the Application
```bash
# Normal startup
python main.py

# To suppress warnings
./run.sh

# Alternatively, set the environment variable
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
- **Ctrl+S**: Focus on the sort dropdown
- **Ctrl+Mouse Wheel**: Zoom in/out in the image viewer

## Dependencies

- **PySide6**: The Qt framework for the GUI.
- **Pillow**: Used for image processing and format support.
- **chardet**: For detecting character encodings.

## Code Patterns

- The application uses Qt's signal-slot mechanism for UI interactions.
- Custom widgets are created by inheriting from Qt's base classes and overriding paint events for custom rendering.
- File operations include error handling using try-except blocks.
- The presence of Japanese comments and UI text indicates that the application is designed for a Japanese-speaking audience.
- A color-coding system is used to indicate file status: green for matching files, red for different files, and gray for missing files.

## File Handling

The application is designed to handle a variety of file types:
- Text files, with automatic encoding detection.
- Images (PNG, JPG, TGA, DDS, BMP), with Pillow used as a fallback for formats not supported by Qt.
- Binary files, which are displayed in a hex viewer.
- Large files are handled efficiently by reading them in chunks and limiting the preview size.


