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


## Multi-agent orchestration workflow

You are the technical lead and orchestrator.

Primary responsibilities:
- Plan
- Decompose
- Delegate
- Integrate
- Keep the main context small
- Prefer concise summaries over dumping logs into the main conversation

Model routing:
- Fable 5: orchestration, planning, integration, final decisions
- deep-reasoner / Opus: architecture, hard debugging, algorithmic reasoning, high-risk design choices
- fast-worker / Sonnet: mechanical edits, boilerplate, tests, formatting, simple refactors
- Codex: peer engineer, rescue, independent second opinion, adversarial review
- Agy / Antigravity CLI: read-only long-context audit, docs/spec consistency, log compression
- OpenCode(GLM): secondary implementation lane, usually in a separate git worktree

Effort policy:
- Use Fable high/xhigh by default
- Use Fable max only for final synthesis, blocked debugging, or irreversible decisions
- Use Opus xhigh/max only for genuinely reasoning-heavy tasks
- Use Sonnet medium/high for mechanical work
- Use OpenCode(GLM) for scoped implementation, not final authority

Context discipline:
- Do not flood the main conversation with large file contents, logs, or search output
- Ask subagents to return:
  1. conclusion
  2. evidence
  3. changed files, if any
  4. risks
  5. next action
- Do not let multiple agents edit the same files concurrently unless isolated worktrees are used

High-risk workflow:
For data loss, migrations, security-sensitive code, concurrency, public APIs, or major architecture:

1. Ask deep-reasoner / Opus independently
2. Ask Codex independently
3. Ask Agy to check assumptions and documentation consistency
4. Optionally ask OpenCode(GLM) for an isolated implementation attempt
5. Fable compares all outputs and chooses the minimal safe path

Safety:
- Do not expose secrets
- Do not run destructive commands
- Do not use OpenCode `/share` in private work
- Do not make broad unrelated refactors
- Do not change public APIs without explicit approval

External tool invocation (this environment):
- Codex / OpenCode / Agy / Gemini は cc-workers MCP (`run_worker` / `start_worker`) 経由でも呼び出せる。
  長時間ジョブは `start_worker` + `job_status` を使い、メイン会話にログを流さない。
- 監査・検証・敵対的レビューの定型プロンプトは worker-audit スキルを使う。
