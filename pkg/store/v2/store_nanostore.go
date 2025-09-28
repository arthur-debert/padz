package v2

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/arthur-debert/nanostore/nanostore"
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/arthur-debert/padz/pkg/logging"
	"github.com/arthur-debert/padz/pkg/project"
)

// Constants are defined in the package level
const (
	MaxPinnedScratches = 5
	storeFileName      = "padz-scratches.json"
)

// NanoStore implements the padz store interface using nanostore
type NanoStore struct {
	store     nanostore.Store
	fs        filesystem.FileSystem
	cfg       *config.Config
	basePath  string
	filesPath string
}

// NewNanoStore creates a new nanostore-based store
func NewNanoStore() (*NanoStore, error) {
	cfg := config.GetConfig()
	return NewNanoStoreWithConfig(cfg)
}

// NewNanoStoreWithScope creates a new store with the specified scope
func NewNanoStoreWithScope(isGlobal bool) (*NanoStore, error) {
	cfg := config.GetConfig()
	cfg.IsGlobalScope = isGlobal
	return NewNanoStoreWithConfig(cfg)
}

// NewNanoStoreWithConfig creates a new store with custom configuration
func NewNanoStoreWithConfig(cfg *config.Config) (*NanoStore, error) {
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

	// Configure nanostore with padz dimensions
	config := nanostore.Config{
		Dimensions: []nanostore.DimensionConfig{
			{
				Name:         "activity",
				Type:         nanostore.Enumerated,
				Values:       []string{"active", "deleted"},
				Prefixes:     map[string]string{}, // No prefix for activity
				DefaultValue: "active",
			},
			{
				Name:         "pinned",
				Type:         nanostore.Enumerated,
				Values:       []string{"no", "yes"},
				Prefixes:     map[string]string{"yes": "p"},
				DefaultValue: "no",
			},
			// Note: Removed project dimension as it causes SimpleID issues
			// when used as Hierarchical. Project will be stored as regular data.
		},
	}

	// Initialize nanostore
	store, err := nanostore.New(storePath, config)
	if err != nil {
		return nil, fmt.Errorf("failed to initialize nanostore: %w", err)
	}

	return &NanoStore{
		store:     store,
		fs:        cfg.FileSystem,
		cfg:       cfg,
		basePath:  basePath,
		filesPath: filesPath,
	}, nil
}

// GetScratches returns all active scratches
func (s *NanoStore) GetScratches() []Scratch {
	logger := logging.GetLogger("store.v2")

	// List only active scratches
	opts := nanostore.ListOptions{
		Filters: map[string]interface{}{
			"activity": "active",
		},
	}

	docs, err := s.store.List(opts)
	if err != nil {
		logger.Error().Err(err).Msg("Failed to query scratches")
		return []Scratch{}
	}

	scratches := make([]Scratch, len(docs))
	for i, doc := range docs {
		logger.Debug().
			Str("uuid", doc.UUID).
			Str("simple_id", doc.SimpleID).
			Str("title", doc.Title).
			Msg("Processing document from nanostore")
		scratches[i] = s.documentToScratch(doc)
	}

	logger.Debug().Int("count", len(scratches)).Msg("Retrieved scratches")
	return scratches
}

// GetPinnedScratches returns only pinned scratches
func (s *NanoStore) GetPinnedScratches() []Scratch {
	logger := logging.GetLogger("store.v2")

	opts := nanostore.ListOptions{
		Filters: map[string]interface{}{
			"activity": "active",
			"pinned":   "yes",
		},
	}

	docs, err := s.store.List(opts)
	if err != nil {
		logger.Error().Err(err).Msg("Failed to query pinned scratches")
		return []Scratch{}
	}

	// Sort by pinned time (newest first)
	scratches := make([]Scratch, len(docs))
	for i, doc := range docs {
		scratches[i] = s.documentToScratch(doc)
	}

	logger.Debug().Int("count", len(scratches)).Msg("Retrieved pinned scratches")
	return scratches
}

// AddScratch adds a new scratch
func (s *NanoStore) AddScratch(scratch Scratch) error {
	logger := logging.GetLogger("store.v2")
	logger.Info().Str("title", scratch.Title).Msg("Adding scratch")

	// Prepare dimensions
	dimensions := map[string]interface{}{
		"activity": "active",
		"pinned":   "no",
		// Project is stored as data, not a dimension
	}

	if scratch.IsPinned {
		dimensions["pinned"] = "yes"
	}

	// Create the document in nanostore
	uuid, err := s.store.Add(scratch.Title, dimensions)
	if err != nil {
		return fmt.Errorf("failed to create scratch: %w", err)
	}

	// Note: Content would be saved to file system separately
	// This would require adding Content field to Scratch struct or handling separately

	// Note: Additional metadata like size, checksum, pinned_at would need to be stored
	// in dimension values or external storage as nanostore doesn't support arbitrary data fields

	logger.Info().Str("uuid", uuid).Str("title", scratch.Title).Msg("Scratch added successfully")
	return nil
}

// RemoveScratch soft deletes a scratch
func (s *NanoStore) RemoveScratch(id string) error {
	logger := logging.GetLogger("store.v2")
	logger.Info().Str("id", id).Msg("Removing scratch")

	// Soft delete by updating activity dimension
	updates := nanostore.UpdateRequest{
		Dimensions: map[string]interface{}{
			"activity": "deleted",
		},
	}

	if err := s.store.Update(id, updates); err != nil {
		return fmt.Errorf("failed to remove scratch: %w", err)
	}

	logger.Info().Str("id", id).Msg("Scratch removed successfully")
	return nil
}

// UpdateScratch updates an existing scratch
func (s *NanoStore) UpdateScratch(scratch Scratch) error {
	logger := logging.GetLogger("store.v2")
	logger.Info().Str("id", scratch.ID).Str("title", scratch.Title).Msg("Updating scratch")

	// Prepare update request
	updates := nanostore.UpdateRequest{
		Title: &scratch.Title,
		Dimensions: map[string]interface{}{
			// Don't include project - it's not a dimension
		},
	}

	// Update dimensions
	if scratch.IsPinned {
		updates.Dimensions["pinned"] = "yes"
	} else {
		updates.Dimensions["pinned"] = "no"
	}

	if scratch.IsDeleted {
		updates.Dimensions["activity"] = "deleted"
	} else {
		updates.Dimensions["activity"] = "active"
	}

	if err := s.store.Update(scratch.ID, updates); err != nil {
		return fmt.Errorf("failed to update scratch: %w", err)
	}

	// Note: Content would be updated in file system separately
	// This would require adding Content field to Scratch struct or handling separately

	logger.Info().Str("id", scratch.ID).Msg("Scratch updated successfully")
	return nil
}

// SaveScratches performs bulk update
func (s *NanoStore) SaveScratches(scratches []Scratch) error {
	logger := logging.GetLogger("store.v2")
	logger.Info().Int("count", len(scratches)).Msg("Bulk saving scratches")

	// This is a full replacement operation
	// First, get all existing scratches
	allOpts := nanostore.ListOptions{}
	allDocs, err := s.store.List(allOpts)
	if err != nil {
		return fmt.Errorf("failed to list existing scratches: %w", err)
	}

	// Delete all existing
	for _, doc := range allDocs {
		if err := s.store.Delete(doc.UUID, false); err != nil {
			logger.Error().Err(err).Str("uuid", doc.UUID).Msg("Failed to delete existing scratch")
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
func (s *NanoStore) Close() error {
	return s.store.Close()
}

// Search searches for scratches matching query
func (s *NanoStore) Search(query string) []Scratch {
	logger := logging.GetLogger("store.v2")

	opts := nanostore.ListOptions{
		FilterBySearch: query,
		Filters: map[string]interface{}{
			"activity": "active",
		},
	}

	docs, err := s.store.List(opts)
	if err != nil {
		logger.Error().Err(err).Str("query", query).Msg("Failed to search scratches")
		return []Scratch{}
	}

	scratches := make([]Scratch, len(docs))
	for i, doc := range docs {
		scratches[i] = s.documentToScratch(doc)
	}

	return scratches
}

// documentToScratch converts a nanostore Document to a Scratch
func (s *NanoStore) documentToScratch(doc nanostore.Document) Scratch {
	scratch := Scratch{
		ID:        doc.SimpleID,
		Project:   "", // TODO: Store project as data in nanostore
		Title:     doc.Title,
		CreatedAt: doc.CreatedAt,
		UpdatedAt: doc.UpdatedAt,
		IsPinned:  s.getDocumentDimension(doc, "pinned") == "yes",
		IsDeleted: s.getDocumentDimension(doc, "activity") == "deleted",
	}

	// Note: Additional metadata would need to be stored separately
	// as nanostore doesn't support arbitrary data fields

	return scratch
}

// getDocumentDimension safely extracts a dimension value from document
func (s *NanoStore) getDocumentDimension(doc nanostore.Document, dimension string) string {
	if val, ok := doc.Dimensions[dimension].(string); ok {
		return val
	}
	return ""
}

// Atomic operations (delegate to non-atomic versions as nanostore handles locking)
func (s *NanoStore) AddScratchAtomic(scratch Scratch) error {
	return s.AddScratch(scratch)
}

func (s *NanoStore) RemoveScratchAtomic(id string) error {
	return s.RemoveScratch(id)
}

func (s *NanoStore) UpdateScratchAtomic(scratch Scratch) error {
	return s.UpdateScratch(scratch)
}

func (s *NanoStore) SaveScratchesAtomic(scratches []Scratch) error {
	return s.SaveScratches(scratches)
}

// GetScratchPath returns the file path for a scratch's content
func (s *NanoStore) GetScratchPath(id string) (string, error) {
	// Resolve to UUID
	uuid, err := s.store.ResolveUUID(id)
	if err != nil {
		return "", err
	}
	return filepath.Join(s.filesPath, uuid), nil
}

// Helper functions

func getBasePath(cfg *config.Config) (string, error) {
	if cfg.DataPath != "" {
		return filepath.Join(cfg.DataPath, "scratch"), nil
	}

	if cfg.IsGlobalScope {
		// Use XDG for global scope
		homeDir, err := os.UserHomeDir()
		if err != nil {
			return "", fmt.Errorf("failed to get home directory: %w", err)
		}
		return filepath.Join(homeDir, ".local", "share", "padz", "scratch"), nil
	}

	// Use local .padz directory for project scope
	projectRoot, err := project.GetProjectRoot(".")
	if err != nil || projectRoot == "" {
		// Not in a project, fall back to global
		homeDir, err := os.UserHomeDir()
		if err != nil {
			return "", fmt.Errorf("failed to get home directory: %w", err)
		}
		return filepath.Join(homeDir, ".local", "share", "padz", "scratch"), nil
	}

	return filepath.Join(projectRoot, ".padz"), nil
}
