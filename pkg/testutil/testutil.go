package testutil

import (
	"fmt"
	"path/filepath"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
)

// SetupTestEnvironment sets up an isolated test environment
// Returns a cleanup function that should be called with defer
func SetupTestEnvironment(t *testing.T) (*config.Config, func()) {
	// Create a memory filesystem for testing
	memFS := filesystem.NewMemoryFileSystem()

	// Create test configuration
	testConfig := &config.Config{
		FileSystem: memFS,
		DataPath:   "/test/data",
	}

	// Save current config
	oldConfig := config.GetConfig()

	// Set test config
	config.SetConfig(testConfig)

	// Create the data directories
	if err := memFS.MkdirAll("/test/data", 0755); err != nil {
		t.Fatalf("Failed to create test data directory: %v", err)
	}
	if err := memFS.MkdirAll("/test/data/scratch", 0755); err != nil {
		t.Fatalf("Failed to create test scratch directory: %v", err)
	}

	// Return cleanup function
	cleanup := func() {
		// Restore original config
		config.SetConfig(oldConfig)
		// Reset the memory filesystem
		memFS.Reset()
	}

	return testConfig, cleanup
}

// GetMemoryFS extracts the memory filesystem from a config
func GetMemoryFS(cfg *config.Config) *filesystem.MemoryFileSystem {
	if memFS, ok := cfg.FileSystem.(*filesystem.MemoryFileSystem); ok {
		return memFS
	}
	return nil
}

// MultiScopeTestEnvironment represents a test environment with multiple scopes
type MultiScopeTestEnvironment struct {
	Config      *config.Config
	MemFS       *filesystem.MemoryFileSystem
	Scopes      map[string]string // scope name -> project directory path
	GlobalDir   string            // global scope directory
	BaseDir     string            // base directory for all test data
	ProjectsDir string            // directory containing project directories
}

// SetupMultiScopeTestEnvironment creates a test environment with multiple scopes
// Each scope gets its own project directory with .padz/scratch/<scope>/ structure
func SetupMultiScopeTestEnvironment(t *testing.T, scopes ...string) (*MultiScopeTestEnvironment, func()) {
	t.Helper()

	// Create a memory filesystem for testing
	memFS := filesystem.NewMemoryFileSystem()

	// Create dynamic test paths
	baseDir := fmt.Sprintf("/test_%d", time.Now().UnixNano())
	dataDir := filepath.Join(baseDir, "data")
	projectsDir := filepath.Join(baseDir, "projects")
	homeDir := filepath.Join(baseDir, "home")

	// Create test configuration
	testConfig := &config.Config{
		FileSystem: memFS,
		DataPath:   dataDir,
	}

	// Save current config
	oldConfig := config.GetConfig()

	// Set test config
	config.SetConfig(testConfig)

	env := &MultiScopeTestEnvironment{
		Config:      testConfig,
		MemFS:       memFS,
		Scopes:      make(map[string]string),
		BaseDir:     baseDir,
		ProjectsDir: projectsDir,
	}

	// Create global scope directory
	env.GlobalDir = filepath.Join(homeDir, ".local", "share", "scratch", "global")
	if err := memFS.MkdirAll(env.GlobalDir, 0755); err != nil {
		t.Fatalf("Failed to create global scratch directory: %v", err)
	}

	// Create project scopes
	for _, scope := range scopes {
		if scope == "global" {
			continue // Already handled above
		}

		// Create project directory structure
		projectDir := filepath.Join(projectsDir, scope)
		padzDir := filepath.Join(projectDir, ".padz", "scratch", scope)

		// Create directories
		if err := memFS.MkdirAll(projectDir, 0755); err != nil {
			t.Fatalf("Failed to create project directory for scope %s: %v", scope, err)
		}
		if err := memFS.MkdirAll(padzDir, 0755); err != nil {
			t.Fatalf("Failed to create .padz directory for scope %s: %v", scope, err)
		}

		// Create .git directory to mark as git repo
		gitDir := filepath.Join(projectDir, ".git")
		if err := memFS.MkdirAll(gitDir, 0755); err != nil {
			t.Fatalf("Failed to create .git directory for scope %s: %v", scope, err)
		}

		env.Scopes[scope] = projectDir
	}

	// Always include global scope
	env.Scopes["global"] = env.GlobalDir

	// Return cleanup function
	cleanup := func() {
		// Restore original config
		config.SetConfig(oldConfig)
		// Reset the memory filesystem
		memFS.Reset()
	}

	return env, cleanup
}

// GetProjectDir returns the project directory for a given scope
func (env *MultiScopeTestEnvironment) GetProjectDir(scope string) string {
	if scope == "global" {
		return env.GlobalDir
	}
	return env.Scopes[scope]
}

// GetScratchDir returns the scratch storage directory for a given scope
func (env *MultiScopeTestEnvironment) GetScratchDir(scope string) string {
	if scope == "global" {
		return env.GlobalDir
	}
	if projectDir, exists := env.Scopes[scope]; exists {
		return filepath.Join(projectDir, ".padz", "scratch", scope)
	}
	return ""
}

// ListScopes returns all available scopes in the test environment
func (env *MultiScopeTestEnvironment) ListScopes() []string {
	scopes := make([]string, 0, len(env.Scopes))
	for scope := range env.Scopes {
		scopes = append(scopes, scope)
	}
	return scopes
}
