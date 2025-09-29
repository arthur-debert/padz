package store

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/arthur-debert/nanostore/nanostore"
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/arthur-debert/padz/pkg/logging"
	"github.com/arthur-debert/padz/pkg/project"
)

// Constants
const (
	MaxPinnedScratches = 5
	storeFileName      = "padz-scratches.json"
)

// Store implements the padz store interface using nanostore
type Store struct {
	store     nanostore.Store
	fs        filesystem.FileSystem
	cfg       *config.Config
	basePath  string
	filesPath string
}

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

func NewStoreWithConfig(cfg *config.Config) (*Store, error) {
	logger := logging.GetLogger("store")
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

	return &Store{
		store:     store,
		fs:        cfg.FileSystem,
		cfg:       cfg,
		basePath:  basePath,
		filesPath: filesPath,
	}, nil
}

func (s *Store) GetScratches() []Scratch {
	return s.GetScratchesWithFilter("", false)
}

// GetScratchesWithFilter returns active scratches filtered by project
func (s *Store) GetScratchesWithFilter(project string, global bool) []Scratch {
	logger := logging.GetLogger("store")

	// List only active scratches, ordered by creation date descending
	opts := nanostore.ListOptions{
		Filters: map[string]interface{}{
			"activity": "active",
		},
		OrderBy: []nanostore.OrderClause{
			{Column: "created_at", Descending: true},
		},
	}

	// Add project filtering at the nanostore level
	if global {
		opts.Filters["_data.project"] = "global"
	} else if project != "" {
		opts.Filters["_data.project"] = project
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

	logger.Debug().Int("count", len(scratches)).Str("project", project).Bool("global", global).Msg("Retrieved filtered scratches")
	return scratches
}

func (s *Store) GetPinnedScratches() []Scratch {
	return s.GetPinnedScratchesWithFilter("", false)
}

// GetPinnedScratchesWithFilter returns pinned scratches filtered by project
func (s *Store) GetPinnedScratchesWithFilter(project string, global bool) []Scratch {
	logger := logging.GetLogger("store")

	opts := nanostore.ListOptions{
		Filters: map[string]interface{}{
			"activity": "active",
			"pinned":   "yes",
		},
		OrderBy: []nanostore.OrderClause{
			{Column: "created_at", Descending: true},
		},
	}

	// Add project filtering at the nanostore level
	if global {
		opts.Filters["_data.project"] = "global"
	} else if project != "" {
		opts.Filters["_data.project"] = project
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

	logger.Debug().Int("count", len(scratches)).Str("project", project).Bool("global", global).Msg("Retrieved pinned scratches")
	return scratches
}

// GetAllScratches returns all scratches (active and deleted)
func (s *Store) GetAllScratches() []Scratch {
	return s.GetAllScratchesWithFilter("", false)
}

// GetAllScratchesWithFilter returns all scratches (active and deleted) filtered by project
func (s *Store) GetAllScratchesWithFilter(project string, global bool) []Scratch {
	logger := logging.GetLogger("store")

	// List all scratches (no activity filter), ordered by creation date descending
	// Note: We can't order by "most recent activity" in SQL since that would require
	// a CASE statement (created_at for active, deleted_at for deleted)
	opts := nanostore.ListOptions{
		Filters: map[string]interface{}{},
		OrderBy: []nanostore.OrderClause{
			{Column: "created_at", Descending: true},
		},
	}

	// Add project filtering at the nanostore level
	if global {
		opts.Filters["_data.project"] = "global"
	} else if project != "" {
		opts.Filters["_data.project"] = project
	}

	docs, err := s.store.List(opts)
	if err != nil {
		logger.Error().Err(err).Msg("Failed to query all scratches")
		return []Scratch{}
	}

	scratches := make([]Scratch, len(docs))
	for i, doc := range docs {
		scratches[i] = s.documentToScratch(doc)
	}

	logger.Debug().Int("count", len(scratches)).Str("project", project).Bool("global", global).Msg("Retrieved all scratches")
	return scratches
}

// GetDeletedScratches returns only deleted scratches
func (s *Store) GetDeletedScratches() []Scratch {
	return s.GetDeletedScratchesWithFilter("", false)
}

// GetDeletedScratchesWithFilter returns only deleted scratches filtered by project
func (s *Store) GetDeletedScratchesWithFilter(project string, global bool) []Scratch {
	logger := logging.GetLogger("store")

	// List only deleted scratches, ordered by deletion date descending (most recently deleted first)
	opts := nanostore.ListOptions{
		Filters: map[string]interface{}{
			"activity": "deleted",
		},
		OrderBy: []nanostore.OrderClause{
			{Column: "_data.deleted_at", Descending: true},
		},
	}

	// Add project filtering at the nanostore level
	if global {
		opts.Filters["_data.project"] = "global"
	} else if project != "" {
		opts.Filters["_data.project"] = project
	}

	docs, err := s.store.List(opts)
	if err != nil {
		logger.Error().Err(err).Msg("Failed to query deleted scratches")
		return []Scratch{}
	}

	scratches := make([]Scratch, len(docs))
	for i, doc := range docs {
		scratches[i] = s.documentToScratch(doc)
	}

	logger.Debug().Int("count", len(scratches)).Str("project", project).Bool("global", global).Msg("Retrieved deleted scratches")
	return scratches
}

// SaveScratches performs bulk update
func (s *Store) SaveScratches(scratches []Scratch) error {
	logger := logging.GetLogger("store")
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

// AddScratch adds a new scratch
func (s *Store) AddScratch(scratch Scratch) error {
	logger := logging.GetLogger("store")
	logger.Info().Str("title", scratch.Title).Msg("Adding scratch")

	// Prepare dimensions and data
	dimensions := map[string]interface{}{
		"activity": "active",
		"pinned":   "no",
	}

	// Handle deleted state
	if scratch.IsDeleted {
		dimensions["activity"] = "deleted"
	}

	if scratch.IsPinned {
		dimensions["pinned"] = "yes"
	}

	// Add project and metadata as data (prefixed with _data.)
	if scratch.Project != "" {
		dimensions["_data.project"] = scratch.Project
	}
	if scratch.Size > 0 {
		dimensions["_data.size"] = scratch.Size
	}
	if scratch.Checksum != "" {
		dimensions["_data.checksum"] = scratch.Checksum
	}
	if !scratch.PinnedAt.IsZero() {
		dimensions["_data.pinned_at"] = scratch.PinnedAt
	}
	if scratch.IsDeleted && scratch.DeletedAt != nil {
		dimensions["_data.deleted_at"] = *scratch.DeletedAt
	}

	// Create the document in nanostore
	uuid, err := s.store.Add(scratch.Title, dimensions)
	if err != nil {
		return fmt.Errorf("failed to create scratch: %w", err)
	}

	// If there's content, update the document with the body
	if scratch.Content != "" {
		updates := nanostore.UpdateRequest{
			Body: &scratch.Content,
		}
		if err := s.store.Update(uuid, updates); err != nil {
			logger.Error().Err(err).Msg("Failed to add content to scratch")
			// Don't fail the whole operation, but log the error
		}
	}

	logger.Info().Str("uuid", uuid).Str("title", scratch.Title).Msg("Scratch added successfully")
	return nil
}

// RemoveScratch soft deletes a scratch
func (s *Store) RemoveScratch(id string) error {
	logger := logging.GetLogger("store")
	logger.Info().Str("id", id).Msg("Removing scratch")

	// Get all current scratches (including deleted ones to find the target)
	allScratches := s.GetAllScratches()

	// Find and update the target scratch
	found := false
	now := time.Now()
	for i := range allScratches {
		if allScratches[i].ID == id {
			allScratches[i].IsDeleted = true
			allScratches[i].DeletedAt = &now
			found = true
			break
		}
	}

	if !found {
		return fmt.Errorf("scratch not found: %s", id)
	}

	// Save all scratches back (this will trigger the nanostore update)
	if err := s.SaveScratchesAtomic(allScratches); err != nil {
		return fmt.Errorf("failed to remove scratch: %w", err)
	}

	logger.Info().Str("id", id).Msg("Scratch removed successfully")
	return nil
}

// UpdateScratch updates an existing scratch
func (s *Store) UpdateScratch(scratch Scratch) error {
	logger := logging.GetLogger("store")
	logger.Info().Str("id", scratch.ID).Str("title", scratch.Title).Msg("Updating scratch")

	// Resolve SimpleID to UUID for update
	uuid, err := s.resolveSimpleIDToUUID(scratch.ID)
	if err != nil {
		return fmt.Errorf("failed to resolve ID %s: %w", scratch.ID, err)
	}

	// Prepare update request
	updates := nanostore.UpdateRequest{
		Title:      &scratch.Title,
		Dimensions: map[string]interface{}{
			// Don't include project - it's not a dimension
		},
	}

	// Update body if content is provided
	if scratch.Content != "" {
		updates.Body = &scratch.Content
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

	// Update data fields
	if scratch.Project != "" {
		updates.Dimensions["_data.project"] = scratch.Project
	}
	if scratch.Size > 0 {
		updates.Dimensions["_data.size"] = scratch.Size
	}
	if scratch.Checksum != "" {
		updates.Dimensions["_data.checksum"] = scratch.Checksum
	}
	if !scratch.PinnedAt.IsZero() {
		updates.Dimensions["_data.pinned_at"] = scratch.PinnedAt
	}
	if scratch.IsDeleted && scratch.DeletedAt != nil {
		updates.Dimensions["_data.deleted_at"] = *scratch.DeletedAt
	}

	if err := s.store.Update(uuid, updates); err != nil {
		return fmt.Errorf("failed to update scratch: %w", err)
	}

	logger.Info().Str("id", scratch.ID).Str("uuid", uuid).Msg("Scratch updated successfully")
	return nil
}

// Close closes the store
func (s *Store) Close() error {
	return s.store.Close()
}

// Search searches for scratches matching query
func (s *Store) Search(query string) []Scratch {
	logger := logging.GetLogger("store")

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
func (s *Store) documentToScratch(doc nanostore.Document) Scratch {
	scratch := Scratch{
		ID:        doc.SimpleID,
		Title:     doc.Title,
		Content:   doc.Body, // Content is stored in nanostore's Body field
		CreatedAt: doc.CreatedAt,
		UpdatedAt: doc.UpdatedAt,
		IsPinned:  s.getDocumentDimension(doc, "pinned") == "yes",
		IsDeleted: s.getDocumentDimension(doc, "activity") == "deleted",
	}

	// Extract data fields from dimensions (prefixed with _data.)
	if project, ok := doc.Dimensions["_data.project"].(string); ok {
		scratch.Project = project
	}
	if size, ok := doc.Dimensions["_data.size"].(float64); ok {
		scratch.Size = int64(size)
	}
	if checksum, ok := doc.Dimensions["_data.checksum"].(string); ok {
		scratch.Checksum = checksum
	}
	if pinnedAt, ok := doc.Dimensions["_data.pinned_at"].(time.Time); ok {
		scratch.PinnedAt = pinnedAt
	}
	// Handle deleted_at
	if scratch.IsDeleted {
		if deletedAt, ok := doc.Dimensions["_data.deleted_at"].(time.Time); ok {
			scratch.DeletedAt = &deletedAt
		}
	}

	return scratch
}

// getDocumentDimension safely extracts a dimension value from document
func (s *Store) getDocumentDimension(doc nanostore.Document, dimension string) string {
	if val, ok := doc.Dimensions[dimension].(string); ok {
		return val
	}
	return ""
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

// GetScratchPath returns the file path for a scratch's content
func (s *Store) GetScratchPath(id string) (string, error) {
	// Resolve to UUID
	uuid, err := s.store.ResolveUUID(id)
	if err != nil {
		return "", err
	}
	return filepath.Join(s.filesPath, uuid), nil
}

// RebuildIndex rebuilds the master index (no-op for nanostore)
func (s *Store) RebuildIndex() error {
	// nanostore handles indexing automatically
	return nil
}

// RunDiscoveryBeforeCommand discovers orphaned files (no-op for nanostore)
func (s *Store) RunDiscoveryBeforeCommand() error {
	// nanostore stores content in the body field, not as separate files
	// so there's no need for discovery of orphaned files
	return nil
}

// resolveSimpleIDToUUID finds the UUID for a given SimpleID by listing all documents
func (s *Store) resolveSimpleIDToUUID(simpleID string) (string, error) {
	// List all documents (including deleted ones) to find the one with this SimpleID
	allOpts := nanostore.ListOptions{}
	docs, err := s.store.List(allOpts)
	if err != nil {
		return "", fmt.Errorf("failed to list documents: %w", err)
	}

	for _, doc := range docs {
		if doc.SimpleID == simpleID {
			return doc.UUID, nil
		}
	}

	return "", fmt.Errorf("scratch not found: %s", simpleID)
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

// Legacy compatibility functions
func GetScratchPath() (string, error) {
	cfg := config.GetConfig()
	return getBasePath(cfg)
}

func GetScratchPathWithConfig(cfg *config.Config) (string, error) {
	return getBasePath(cfg)
}

func GetScratchFilePath(id string) (string, error) {
	cfg := config.GetConfig()
	return GetScratchFilePathWithConfig(id, cfg)
}

func GetScratchFilePathWithConfig(id string, cfg *config.Config) (string, error) {
	store, err := NewStoreWithConfig(cfg)
	if err != nil {
		return "", err
	}
	defer func() { _ = store.Close() }()

	return store.GetScratchPath(id)
}

// GetTestStore returns the underlying nanostore as TestStore for testing purposes
func (s *Store) GetTestStore() (nanostore.TestStore, bool) {
	if testStore, ok := s.store.(nanostore.TestStore); ok {
		return testStore, true
	}
	return nil, false
}

// ResolveIDToUUID resolves any ID (SimpleID or UUID) to a full UUID
func (s *Store) ResolveIDToUUID(id string) (string, error) {
	return s.store.ResolveUUID(id)
}

// GetScratchByUUID retrieves a scratch by its UUID
func (s *Store) GetScratchByUUID(uuid string) (*Scratch, error) {
	// List all documents to find the one with this UUID
	allOpts := nanostore.ListOptions{}
	docs, err := s.store.List(allOpts)
	if err != nil {
		return nil, fmt.Errorf("failed to list documents: %w", err)
	}

	for _, doc := range docs {
		if doc.UUID == uuid {
			scratch := s.documentToScratch(doc)
			return &scratch, nil
		}
	}

	return nil, fmt.Errorf("scratch not found: %s", uuid)
}
