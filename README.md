# PADZ: Command-Line Note-Taking Tool

A simple, fast command-line note-taking tool designed for developers who want quick, context-aware scratch notes without leaving their terminal environment.

## Architecture Overview

**PADZ** follows a simple, effective architecture designed for personal productivity:

- **Central Git Repository Storage**: All notes stored in a single location separate from project repos
- **JSON-Based Metadata**: Human-readable metadata format for easy inspection and debugging
- **Target Capacity**: Optimized for ~1,000 tasks without requiring complex indexing
- **Single Instance Design**: No multi-user or concurrent access complexity
- **Simple Error Handling**: Direct git error messages without complex wrapping
- **Future Export Support**: Planned markdown lists and directory structure export

## What You Get

### 📁 Project Structure
```
padz/
├── cmd/padz/    # CLI entry point with Cobra commands
│   ├── main.go                # Main application entry
│   └── root.go                # Root command configuration
├── pkg/                       # Reusable packages
│   └── logging/               # Structured logging setup
├── scripts/                   # Build and development scripts
├── .github/workflows/         # GitHub Actions CI/CD
├── .goreleaser.yml           # Multi-platform release configuration
└── go.mod                    # Go module definition
```

### 🛠️ Build & Development Scripts
- **`./scripts/build`** - Builds the CLI binary with embedded version information
- **`./scripts/test`** - Runs tests with race detection and coverage
- **`./scripts/test-with-coverage`** - Detailed coverage report with visualization
- **`./scripts/lint`** - Comprehensive code linting with golangci-lint
- **`./scripts/pre-commit`** - Git hooks for code quality enforcement
- **`./scripts/release-new`** - Automated semantic versioning and releases
- **`./scripts/cloc-go`** - Go-specific line counting statistics

### 🚀 GitHub Actions Workflows
- **Test workflow** - Runs on every push: build, test, and coverage upload
- **Release workflow** - Triggers on version tags: multi-platform builds and GitHub releases
- **Codecov integration** - Automatic coverage reporting

### 📦 Release & Distribution
- **GoReleaser** configuration for:
  - Linux, macOS, Windows binaries (amd64, arm64)
  - Homebrew formula generation
  - Debian packages (.deb)
  - Checksums and release notes
- **Homebrew tap** support with debug mode for testing

### 🔧 Pre-configured Features
- **Cobra CLI framework** with command structure
- **Structured logging** with zerolog
- **Version command** with Git commit information
- **Comprehensive error handling**
- **Context-aware configuration**
- **Pre-commit hooks** for consistent code quality

### 📝 Logging

padz features comprehensive dual logging that provides both user-friendly console output and detailed file logging for debugging:

**Console Logging** (respects verbosity flags):
- Use `-v` for Info level, `-vv` for Debug level, `-vvv` for Trace level
- Human-readable format with colors and timestamps
- Output goes to stderr for proper piping behavior

**File Logging** (always enabled):
- **All activity is logged** to file regardless of verbosity settings
- JSON format with structured fields for easy parsing
- **Location**: 
  - **macOS**: `~/Library/Application Support/padz/padz.log`
  - **Linux**: `~/.local/state/padz/padz.log` 
  - **Windows**: `%LOCALAPPDATA%\padz\padz.log`
- Automatic directory creation with proper permissions
- Complete audit trail of all commands, errors, and debug information

**Finding Your Log File**:
```bash
# Run any padz command with debug verbosity to see the log file location
padz ls -vv
# Look for: "Logger initialized with dual output log_file=..."

# Or check the standard locations above
```

This dual logging approach ensures you always have detailed information available for troubleshooting while keeping console output clean and user-friendly.

### 🎯 Development Tools
- **golangci-lint** - Comprehensive Go linting (auto-installed)
- **gotestsum** - Better test output formatting (auto-installed)
- **Race detection** enabled in tests
- **Coverage reporting** with HTML output
- **Semantic versioning** automation

## Quick Start Commands

After adding this module:

```bash
# Build your CLI
./scripts/build
./bin/padz --version

# Run tests
./scripts/test

# Set up development environment
./scripts/pre-commit install

# Create a release
./scripts/release-new --patch
```

## Configuration

The module is pre-configured with:
- Go 1.23+ support
- MIT license
- GitHub Actions for CI/CD
- Codecov for coverage tracking
- Homebrew formula generation