# AI Agent Guide for DiffLoupe Project

This document outlines the general workflow and provides an overview of the DiffLoupe project for AI agents.

## Project Overview

DiffLoupe is a cross-platform GUI application designed for comparing folders and files. It offers a visual interface to identify differences in directory structures and file contents. The project has two main implementations:

*   **C++ Version (Primary Focus)**: The core application is developed in C++ using the Qt framework. This is currently the main development focus.
*   **Python/PySide6 Version**: A Python implementation using PySide6 is also available, providing similar functionality.

The application supports various comparison modes including text diff, image preview, and hex viewer for binary files.

## AI Agent Workflow

When working on tasks within this project, please follow this general workflow:

1.  **Understand**:
    *   Thoroughly analyze the user's request and the relevant codebase context.
    *   Use `read_file`, `read_many_files`, `search_file_content`, and `glob` tools to gather necessary information about file structures, existing code patterns, and conventions.
    *   Consult `AGENTS.md` for general project guidelines, and `CPP_GUIDE.md` (for C++ tasks) or `Python/CLAUDE.md` / `Python/GEMINI.md` (for Python tasks) for version-specific instructions.

2.  **Plan**:
    *   Formulate a clear and concise plan based on your understanding.
    *   Consider the impact of your changes on existing code, tests, and configurations.
    *   If applicable, include steps for self-verification (e.g., writing temporary tests or using debug statements).
    *   If the task is complex or ambiguous, propose your plan to the user for confirmation before proceeding.

3.  **Implement**:
    *   Execute the plan using the available tools (`replace`, `write_file`, `run_shell_command`, etc.).
    *   Strictly adhere to the project's established conventions, coding style, and architectural patterns.
    *   Ensure changes are idiomatic to the existing codebase.
    *   Add comments sparingly, focusing on *why* a change was made, especially for complex logic.

4.  **Verify**:
    *   After making changes, verify their correctness and quality.
    *   Run relevant tests (unit, integration, etc.) if available. Identify test commands by examining `README` files, build configurations, or existing test execution patterns.
    *   Execute project-specific build, linting, and type-checking commands (e.g., `cmake && make`, `ruff check`, `tsc`).
    *   For macOS C++ builds, ensure the `macdeployqt` step is correctly performed to create a functional application bundle.

By following this workflow, you can ensure efficient, safe, and high-quality contributions to the DiffLoupe project.

## Language Preference

Gemini will interact in Japanese.
