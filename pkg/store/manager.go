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
		return cfg.FileSystem.Join(cfg.DataPath, "scratch", "global"), nil
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

	// Create a new config with the specific storage path
	storeConfig := &config.Config{
		FileSystem: cfg.FileSystem,
		DataPath:   storagePath,
	}

	return NewStoreWithConfig(storeConfig)
}
