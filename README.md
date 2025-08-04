# Go CLI Module

This module sets up a complete Go CLI application with Cobra framework, comprehensive build tooling, and CI/CD pipelines.

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
- **`./scripts/build`** - Builds the CLI binary with embedded version info
- **`./scripts/test`** - Runs tests with race detection and coverage
- **`./scripts/test-with-coverage`** - Detailed coverage report with visualization
- **`./scripts/lint`** - Comprehensive code linting with golangci-lint
- **`./scripts/pre-commit`** - Git hooks for code quality enforcement
- **`./scripts/release-new`** - Automated semantic versioning and releases
- **`./scripts/cloc-go`** - Go-specific line counting statistics

### 🚀 GitHub Actions Workflows
- **Test workflow** - Runs on every push: build, test, coverage upload
- **Release workflow** - Triggers on version tags: multi-platform builds, GitHub releases
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
- **Version command** with git commit info
- **Comprehensive error handling**
- **Context-aware configuration**
- **Pre-commit hooks** for consistent code quality

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