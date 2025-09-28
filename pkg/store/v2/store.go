package v2

import (
	"fmt"
	"path/filepath"
	"strings"
	"time"

	"github.com/arthur-debert/nanostore/nanostore/api"
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/arthur-debert/padz/pkg/logging"
	"github.com/arthur-debert/padz/pkg/project"
)

const (
	MaxPinnedScratches = 5
	storeFileName      = "padz-scratches.json"
)

// Store implements the padz store interface using nanostore
type Store struct {
	store     *api.TypedStore[PadzScratch]
	fs        filesystem.FileSystem
	cfg       *config.Config
	basePath  string
	filesPath string
}

// NewStore creates a new nanostore-based store
func NewStore() (*Store, error) {
	cfg := config.GetConfig()
	return NewStoreWithConfig(cfg)
}

// NewStoreWithScope creates a new store with the specified scope
func NewStoreWithScope(isGlobal bool) (*Store, error) {
	cfg := config.GetConfig()
	cfg.IsGlobalScope = isGlobal
	return NewStoreWithConfig(cfg)
}

// NewStoreWithConfig creates a new store with custom configuration
func NewStoreWithConfig(cfg *config.Config) (*Store, error) {
	logger := logging.GetLogger("store.v2")
	logger.Info().Bool("is_global", cfg.IsGlobalScope).Msg("Initializing nanostore")

	basePath, err := getBasePath(cfg)
	if err != nil {
		return nil, err
	}

	// Ensure base directory exists
	if err := cfg.FileSystem.MkdirAll(basePath, 0755); err != nil {
		return nil, fmt.Errorf("failed to create base directory: %w", err)
	}

	// Create files subdirectory
	filesPath := filepath.Join(basePath, "files")
	if err := cfg.FileSystem.MkdirAll(filesPath, 0755); err != nil {
		return nil, fmt.Errorf("failed to create files directory: %w", err)
	}

	storePath := filepath.Join(basePath, storeFileName)

	// Initialize nanostore with canonical view (active items)
	typedStore, err := api.NewFromType[PadzScratch](storePath,
		api.WithCanonicalView(func() api.TypedQuery[PadzScratch] {
			return api.NewTypedQuery[PadzScratch]().Status("active")
		}),
		// TODO: Add FileSystem adapter when API is available
		// api.WithFileSystem(adaptFileSystem(cfg.FileSystem)),
	)
	if err != nil {
		return nil, fmt.Errorf("failed to initialize nanostore: %w", err)
	}

	return &Store{
		store:     typedStore,
		fs:        cfg.FileSystem,
		cfg:       cfg,
		basePath:  basePath,
		filesPath: filesPath,
	}, nil
}

// GetScratches returns all active scratches
func (s *Store) GetScratches() []Scratch {
	logger := logging.GetLogger("store.v2")

	// Query for active scratches (canonical view)
	results, err := s.store.Query().Find()
	if err != nil {
		logger.Error().Err(err).Msg("Failed to query scratches")
		return []Scratch{}
	}

	scratches := make([]Scratch, len(results))
	for i, ps := range results {
		scratch := ps.ToScratch()
		// Use the simple ID from nanostore
		scratch.ID = ps.SimpleID
		scratches[i] = scratch
	}

	logger.Debug().Int("count", len(scratches)).Msg("Retrieved scratches")
	return scratches
}

// GetPinnedScratches returns only pinned scratches
func (s *Store) GetPinnedScratches() []Scratch {
	logger := logging.GetLogger("store.v2")

	results, err := s.store.Query().
		Pinned("yes").
		OrderBy("pinned_at", "desc").
		Find()
	if err != nil {
		logger.Error().Err(err).Msg("Failed to query pinned scratches")
		return []Scratch{}
	}

	scratches := make([]Scratch, len(results))
	for i, ps := range results {
		scratch := ps.ToScratch()
		scratch.ID = ps.SimpleID
		scratches[i] = scratch
	}

	logger.Debug().Int("count", len(scratches)).Msg("Retrieved pinned scratches")
	return scratches
}

// AddScratch adds a new scratch
func (s *Store) AddScratch(scratch Scratch) error {
	logger := logging.GetLogger("store.v2")
	logger.Info().Str("title", scratch.Title).Msg("Adding scratch")

	ps := FromScratch(scratch)

	// Save content to file
	if scratch.ID != "" && len(ps.Content) > 0 {
		contentPath := filepath.Join(s.filesPath, scratch.ID)
		if err := s.fs.WriteFile(contentPath, []byte(ps.Content), 0644); err != nil {
			return fmt.Errorf("failed to save content: %w", err)
		}
		ps.Content = "" // Don't store content in nanostore
	}

	id, err := s.store.Create(scratch.Title, ps)
	if err != nil {
		return fmt.Errorf("failed to create scratch: %w", err)
	}

	logger.Info().Str("id", id).Str("title", scratch.Title).Msg("Scratch added successfully")
	return nil
}

// RemoveScratch soft deletes a scratch
func (s *Store) RemoveScratch(id string) error {
	logger := logging.GetLogger("store.v2")
	logger.Info().Str("id", id).Msg("Removing scratch")

	// Resolve ID to UUID
	uuid, err := s.resolveID(id)
	if err != nil {
		return err
	}

	// Get the scratch to update
	ps, err := s.store.Get(uuid)
	if err != nil {
		return fmt.Errorf("failed to get scratch: %w", err)
	}

	// Soft delete
	ps.Status = "deleted"
	ps.DeletedAt = time.Now()

	if err := s.store.Update(uuid, ps); err != nil {
		return fmt.Errorf("failed to update scratch: %w", err)
	}

	logger.Info().Str("id", id).Str("uuid", uuid).Msg("Scratch removed successfully")
	return nil
}

// UpdateScratch updates an existing scratch
func (s *Store) UpdateScratch(scratch Scratch) error {
	logger := logging.GetLogger("store.v2")
	logger.Info().Str("id", scratch.ID).Str("title", scratch.Title).Msg("Updating scratch")

	// Resolve ID to UUID
	uuid, err := s.resolveID(scratch.ID)
	if err != nil {
		return err
	}

	// Get existing scratch
	existing, err := s.store.Get(uuid)
	if err != nil {
		return fmt.Errorf("failed to get existing scratch: %w", err)
	}

	// Update fields
	existing.Title = scratch.Title
	existing.Project = scratch.Project
	existing.Size = scratch.Size
	existing.Checksum = scratch.Checksum
	existing.UpdatedAt = time.Now()

	// Update dimensions
	if scratch.IsPinned {
		existing.Pinned = "yes"
		existing.PinnedAt = scratch.PinnedAt
	} else {
		existing.Pinned = "no"
		existing.PinnedAt = time.Time{}
	}

	if scratch.IsDeleted {
		existing.Status = "deleted"
		if scratch.DeletedAt != nil {
			existing.DeletedAt = *scratch.DeletedAt
		}
	} else {
		existing.Status = "active"
		existing.DeletedAt = time.Time{}
	}

	if err := s.store.Update(uuid, existing); err != nil {
		return fmt.Errorf("failed to update scratch: %w", err)
	}

	logger.Info().Str("id", scratch.ID).Str("uuid", uuid).Msg("Scratch updated successfully")
	return nil
}

// SaveScratches performs bulk update
func (s *Store) SaveScratches(scratches []Scratch) error {
	logger := logging.GetLogger("store.v2")
	logger.Info().Int("count", len(scratches)).Msg("Bulk saving scratches")

	// This is a full replacement operation
	// First, get all existing scratches (including deleted)
	allExisting, err := s.store.Query().
		StatusIn("active", "deleted").
		Find()
	if err != nil {
		return fmt.Errorf("failed to query existing scratches: %w", err)
	}

	// Delete all existing
	for _, ps := range allExisting {
		if err := s.store.Delete(ps.UUID, false); err != nil {
			logger.Error().Err(err).Str("uuid", ps.UUID).Msg("Failed to delete existing scratch")
		}
	}

	// Add all new scratches
	for _, scratch := range scratches {
		if err := s.AddScratch(scratch); err != nil {
			logger.Error().Err(err).Str("title", scratch.Title).Msg("Failed to add scratch in bulk save")
		}
	}

	logger.Info().Int("count", len(scratches)).Msg("Bulk save completed")
	return nil
}

// Close closes the store
func (s *Store) Close() error {
	return s.store.Close()
}

// resolveID converts user-facing IDs to UUIDs
func (s *Store) resolveID(id string) (string, error) {
	// Try to get by simple ID first
	results, err := s.store.Query().
		StatusIn("active", "deleted").
		Find()
	if err != nil {
		return "", err
	}

	// Check if it's a simple numeric ID or pinned ID
	for _, ps := range results {
		if ps.SimpleID == id {
			return ps.UUID, nil
		}
	}

	// Check if it's a UUID
	for _, ps := range results {
		if ps.UUID == id || strings.HasPrefix(ps.UUID, id) {
			return ps.UUID, nil
		}
	}

	return "", fmt.Errorf("scratch not found: %s", id)
}

// Atomic operations (delegate to non-atomic versions as nanostore handles locking)
func (s *Store) AddScratchAtomic(scratch Scratch) error {
	return s.AddScratch(scratch)
}

func (s *Store) RemoveScratchAtomic(id string) error {
	return s.RemoveScratch(id)
}

func (s *Store) UpdateScratchAtomic(scratch Scratch) error {
	return s.UpdateScratch(scratch)
}

func (s *Store) SaveScratchesAtomic(scratches []Scratch) error {
	return s.SaveScratches(scratches)
}

// Helper functions

func getBasePath(cfg *config.Config) (string, error) {
	if cfg.DataPath != "" {
		return filepath.Join(cfg.DataPath, "scratch"), nil
	}

	if cfg.IsGlobalScope {
		// Use XDG for global scope
		return filepath.Join(cfg.FileSystem.HomeDir(), ".local", "share", "padz", "scratch"), nil
	}

	// Use local .padz directory for project scope
	projectRoot, err := project.GetProjectRoot(".")
	if err != nil || projectRoot == "" {
		// Not in a project, fall back to global
		return filepath.Join(cfg.FileSystem.HomeDir(), ".local", "share", "padz", "scratch"), nil
	}

	return filepath.Join(projectRoot, ".padz"), nil
}

// adaptFileSystem adapts padz FileSystem to nanostore's interface
func adaptFileSystem(fs filesystem.FileSystem) interface{} {
	// For now, return nil to use nanostore's default
	// TODO: Implement adapter if needed
	return nil
}
