package store

import (
	"fmt"
	"path/filepath"
	"sync"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/project"
)

// StoreManager manages multiple Store instances for different scopes
type StoreManager struct {
	stores map[string]*Store
	mu     sync.RWMutex
}

// NewStoreManager creates a new StoreManager instance
func NewStoreManager() *StoreManager {
	return &StoreManager{
		stores: make(map[string]*Store),
	}
}

// GetGlobalStore returns the global store, creating it if necessary
func (sm *StoreManager) GetGlobalStore() (*Store, error) {
	return sm.getStore("global", "")
}

// GetProjectStore returns a store for the given project scope, creating it if necessary
func (sm *StoreManager) GetProjectStore(scope, projectDir string) (*Store, error) {
	if scope == "global" {
		return nil, fmt.Errorf("use GetGlobalStore() for global scope")
	}
	return sm.getStore(scope, projectDir)
}

// GetStore returns a Store for the given scope, creating it if necessary
// Deprecated: Use GetGlobalStore() or GetProjectStore() for clarity
func (sm *StoreManager) GetStore(scope, projectDir string) (*Store, error) {
	return sm.getStore(scope, projectDir)
}

// getStore is the internal implementation shared by public methods
func (sm *StoreManager) getStore(scope, projectDir string) (*Store, error) {
	sm.mu.RLock()
	if store, exists := sm.stores[scope]; exists {
		sm.mu.RUnlock()
		return store, nil
	}
	sm.mu.RUnlock()

	// Create new store
	storePath, err := sm.getStorePath(scope, projectDir)
	if err != nil {
		return nil, fmt.Errorf("failed to get store path for scope %s: %w", scope, err)
	}

	store, err := NewStoreAtPath(storePath)
	if err != nil {
		return nil, fmt.Errorf("failed to create store for scope %s: %w", scope, err)
	}

	sm.mu.Lock()
	sm.stores[scope] = store
	sm.mu.Unlock()

	return store, nil
}

// GetCurrentStore returns a Store for the current scope based on working directory
func (sm *StoreManager) GetCurrentStore(workingDir string, globalFlag bool) (*Store, string, error) {
	var scope string
	var store *Store
	var err error

	if globalFlag {
		scope = "global"
		store, err = sm.GetGlobalStore()
	} else {
		scope, err = project.GetCurrentProject(workingDir)
		if err != nil {
			return nil, "", fmt.Errorf("failed to get current project: %w", err)
		}

		if scope == "global" {
			store, err = sm.GetGlobalStore()
		} else {
			store, err = sm.GetProjectStore(scope, workingDir)
		}
	}

	if err != nil {
		return nil, "", err
	}

	return store, scope, nil
}

// ListScopes returns all currently loaded scopes
func (sm *StoreManager) ListScopes() []string {
	sm.mu.RLock()
	defer sm.mu.RUnlock()

	scopes := make([]string, 0, len(sm.stores))
	for scope := range sm.stores {
		scopes = append(scopes, scope)
	}
	return scopes
}

// getStorePath determines the storage path for a given scope
func (sm *StoreManager) getStorePath(scope, projectDir string) (string, error) {
	cfg := config.GetConfig()

	if scope == "global" {
		// Global scope: use configured data path with global subdirectory
		return cfg.FileSystem.Join(cfg.DataPath, "global"), nil
	}

	// Project scope: find git root and use .padz directory
	gitRoot, err := sm.findGitRoot(projectDir)
	if err != nil {
		return "", fmt.Errorf("failed to find git root for project scope %s: %w", scope, err)
	}

	return cfg.FileSystem.Join(gitRoot, ".padz", "scratch", scope), nil
}

// findGitRoot traverses up the directory tree to find the git repository root
func (sm *StoreManager) findGitRoot(startDir string) (string, error) {
	cfg := config.GetConfig()
	fs := cfg.FileSystem

	dir := startDir
	for {
		gitDir := fs.Join(dir, ".git")
		if _, err := fs.Stat(gitDir); err == nil {
			return dir, nil
		}

		parent := filepath.Dir(dir)
		if parent == dir {
			return "", fmt.Errorf("no git repository found in directory tree starting from %s", startDir)
		}
		dir = parent
	}
}

// NewStoreAtPath creates a new Store instance at the specified path
func NewStoreAtPath(storagePath string) (*Store, error) {
	cfg := config.GetConfig()

	// The issue is that store.load() calls getMetadataPathWithConfig which calls GetScratchPathWithConfig(cfg)
	// GetScratchPathWithConfig does: cfg.FileSystem.Join(cfg.DataPath, "scratch")
	// So we need cfg.DataPath such that when "scratch" is appended, we get storagePath
	//
	// For storagePath="/test/projects/webapp/.padz/scratch/webapp", we need:
	// cfg.DataPath="/test/projects/webapp/.padz/scratch/webapp" - "scratch" = "/test/projects/webapp/.padz" + "/webapp"
	// Actually, let's back up: what we want is for GetScratchPathWithConfig to return storagePath exactly
	//
	// GetScratchPathWithConfig returns: cfg.DataPath + "/scratch"
	// So if we want it to return storagePath, we need: cfg.DataPath = storagePath - "/scratch"
	//
	// For storagePath="/test/projects/webapp/.padz/scratch/webapp"
	// We need cfg.DataPath="/test/projects/webapp/.padz/scratch/webapp" with no "scratch" appended
	// But the function always appends "scratch", so we need:
	// cfg.DataPath = "/test/projects/webapp/.padz" and the function will make it "/test/projects/webapp/.padz/scratch"
	// But we want "/test/projects/webapp/.padz/scratch/webapp", so this doesn't work.
	//
	// The issue is that GetScratchPathWithConfig assumes a certain directory structure.
	// Let me instead create a config where DataPath is set such that DataPath+"scratch" = storagePath

	// If storagePath = "/a/b/c", we want DataPath such that DataPath + "scratch" = "/a/b/c"
	// So DataPath = "/a/b/c" - "scratch"
	// We need to remove the "scratch" suffix if it exists, or set parent appropriately

	// Check if storagePath ends with "scratch" - if so, use the parent
	var dataPath string
	if filepath.Base(storagePath) == "scratch" {
		// storagePath = "/a/b/scratch", so DataPath = "/a/b"
		dataPath = filepath.Dir(storagePath)
	} else {
		// storagePath = "/a/b/webapp", we want DataPath + "scratch" = "/a/b/webapp"
		// This means DataPath = "/a/b/webapp" - "scratch"
		// Since "scratch" is appended as a path component, we need:
		// DataPath = parent of (storagePath - last component) + remaining
		// This is complex. Let me use a simpler approach:
		// Set DataPath to storagePath itself, and the function will create storagePath/scratch
		// But that's not what we want.
		//
		// Actually, let me just override the "scratch" part. Since we can't easily make
		// GetScratchPathWithConfig return the exact path, let's set:
		dataPath = filepath.Dir(storagePath) // Parent directory
	}

	// Create a custom config
	customConfig := &config.Config{
		FileSystem: cfg.FileSystem,
		DataPath:   dataPath,
	}

	// Create the Store with custom config
	store := &Store{
		fs:  cfg.FileSystem,
		cfg: customConfig,
	}

	// Initialize metadata manager with the exact storage path
	store.metadataManager = NewMetadataManager(cfg.FileSystem, storagePath)

	// Force new metadata system for stores created with explicit paths
	// This avoids the legacy path resolution issues in GetScratchPathWithConfig
	store.useNewMetadata = true

	if err := store.load(); err != nil {
		return nil, err
	}

	return store, nil
}
