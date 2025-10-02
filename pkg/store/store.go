package store

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/arthur-debert/nanostore/nanostore"
	"github.com/arthur-debert/nanostore/nanostore/api"
	"github.com/arthur-debert/nanostore/types"
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/logging"
	"github.com/arthur-debert/padz/pkg/project"
)

// Constants
const (
	MaxPinnedScratches = 5
	storeFileName      = "padz-scratches.json"
)

// Store implements the padz store interface using nanostore TypedAPI
type Store struct {
	store    *api.TypedStore[TypedScratch]
	cfg      *config.Config
	basePath string
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
	logger.Info().Bool("is_global", cfg.IsGlobalScope).Msg("Initializing nanostore with TypedAPI")

	basePath, err := getBasePath(cfg)
	if err != nil {
		return nil, err
	}

	// Ensure base directory exists
	if err := os.MkdirAll(basePath, 0755); err != nil {
		return nil, fmt.Errorf("failed to create base directory: %w", err)
	}

	storePath := filepath.Join(basePath, storeFileName)

	// Initialize TypedStore - configuration is automatically generated from TypedScratch struct tags
	store, err := api.NewFromType[TypedScratch](storePath)
	if err != nil {
		return nil, fmt.Errorf("failed to initialize nanostore TypedStore: %w", err)
	}

	return &Store{
		store:    store,
		cfg:      cfg,
		basePath: basePath,
	}, nil
}

func (s *Store) GetScratches() []Scratch {
	return s.GetScratchesWithFilter("", false)
}

// GetScratchesWithFilter returns active scratches filtered by project
func (s *Store) GetScratchesWithFilter(project string, global bool) []Scratch {
	logger := logging.GetLogger("store")

	// Use TypedAPI List with type-safe filtering
	opts := types.ListOptions{
		Filters: map[string]interface{}{
			"activity": "active",
		},
		OrderBy: []types.OrderClause{
			{Column: "created_at", Descending: true},
		},
	}

	// Add project filtering at the nanostore level
	if global {
		opts.Filters["_data.Project"] = "global"
	} else if project != "" {
		opts.Filters["_data.Project"] = project
	}

	typedScratches, err := s.store.List(opts)
	if err != nil {
		logger.Error().Err(err).Msg("Failed to query scratches")
		return []Scratch{}
	}

	// Convert TypedScratch to legacy Scratch structs
	scratches := make([]Scratch, len(typedScratches))
	for i, ts := range typedScratches {
		logger.Debug().
			Str("uuid", ts.UUID).
			Str("simple_id", ts.SimpleID).
			Str("title", ts.Title).
			Msg("Processing typed scratch from nanostore")
		scratches[i] = ts.ToScratch()
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

	opts := types.ListOptions{
		Filters: map[string]interface{}{
			"activity": "active",
			"pinned":   "yes",
		},
		OrderBy: []types.OrderClause{
			{Column: "created_at", Descending: true},
		},
	}

	// Add project filtering at the nanostore level
	if global {
		opts.Filters["_data.Project"] = "global"
	} else if project != "" {
		opts.Filters["_data.Project"] = project
	}

	typedScratches, err := s.store.List(opts)
	if err != nil {
		logger.Error().Err(err).Msg("Failed to query pinned scratches")
		return []Scratch{}
	}

	// Convert TypedScratch to legacy Scratch structs
	scratches := make([]Scratch, len(typedScratches))
	for i, ts := range typedScratches {
		scratches[i] = ts.ToScratch()
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
	opts := types.ListOptions{
		Filters: map[string]interface{}{},
		OrderBy: []types.OrderClause{
			{Column: "created_at", Descending: true},
		},
	}

	// Add project filtering at the nanostore level
	if global {
		opts.Filters["_data.Project"] = "global"
	} else if project != "" {
		opts.Filters["_data.Project"] = project
	}

	typedScratches, err := s.store.List(opts)
	if err != nil {
		logger.Error().Err(err).Msg("Failed to query all scratches")
		return []Scratch{}
	}

	scratches := make([]Scratch, len(typedScratches))
	for i, ts := range typedScratches {
		scratches[i] = ts.ToScratch()
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
	opts := types.ListOptions{
		Filters: map[string]interface{}{
			"activity": "deleted",
		},
		OrderBy: []types.OrderClause{
			{Column: "_data.DeletedAt", Descending: true},
		},
	}

	// Add project filtering at the nanostore level
	if global {
		opts.Filters["_data.Project"] = "global"
	} else if project != "" {
		opts.Filters["_data.Project"] = project
	}

	typedScratches, err := s.store.List(opts)
	if err != nil {
		logger.Error().Err(err).Msg("Failed to query deleted scratches")
		return []Scratch{}
	}

	scratches := make([]Scratch, len(typedScratches))
	for i, ts := range typedScratches {
		scratches[i] = ts.ToScratch()
	}

	logger.Debug().Int("count", len(scratches)).Str("project", project).Bool("global", global).Msg("Retrieved deleted scratches")
	return scratches
}

// SaveScratches performs bulk update
func (s *Store) SaveScratches(scratches []Scratch) error {
	logger := logging.GetLogger("store")
	logger.Info().Int("count", len(scratches)).Msg("Bulk saving scratches")

	// This is a full replacement operation
	// First, get all existing scratches using TypedAPI
	allOpts := types.ListOptions{}
	allTypedScratches, err := s.store.List(allOpts)
	if err != nil {
		return fmt.Errorf("failed to list existing scratches: %w", err)
	}

	// Delete all existing using UUIDs
	for _, ts := range allTypedScratches {
		if err := s.store.Delete(ts.UUID, false); err != nil {
			logger.Error().Err(err).Str("uuid", ts.UUID).Msg("Failed to delete existing scratch")
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

	// Convert legacy Scratch to TypedScratch (excluding Body for now)
	typedScratch := FromScratch(scratch)

	// Create the document using TypedStore API (without body)
	simpleID, err := s.store.Create(scratch.Title, typedScratch)
	if err != nil {
		return fmt.Errorf("failed to create scratch: %w", err)
	}

	// If we have content, update the document to include the body
	if scratch.Content != "" {
		// Get the created document and update its body
		createdScratch, err := s.store.Get(simpleID)
		if err != nil {
			return fmt.Errorf("failed to get created scratch for body update: %w", err)
		}
		createdScratch.Body = scratch.Content

		// Update with the body
		if err := s.store.Update(simpleID, createdScratch); err != nil {
			return fmt.Errorf("failed to update scratch with body: %w", err)
		}
	}

	logger.Info().Str("simple_id", simpleID).Str("title", scratch.Title).Msg("Scratch added successfully")
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

	// Convert legacy Scratch to TypedScratch
	typedScratch := FromScratch(scratch)

	// Update using TypedStore API - uses SimpleID directly
	if err := s.store.Update(scratch.ID, typedScratch); err != nil {
		return fmt.Errorf("failed to update scratch: %w", err)
	}

	logger.Info().Str("id", scratch.ID).Str("title", scratch.Title).Msg("Scratch updated successfully")
	return nil
}

// Close closes the store
func (s *Store) Close() error {
	return s.store.Close()
}

// Search searches for scratches matching query
func (s *Store) Search(query string) []Scratch {
	return s.SearchWithFilter(query, "", false)
}

// SearchWithFilter searches for scratches matching query with project and scope filtering
func (s *Store) SearchWithFilter(query, project string, global bool) []Scratch {
	logger := logging.GetLogger("store")

	opts := types.ListOptions{
		FilterBySearch: query,
		Filters: map[string]interface{}{
			"activity": "active",
		},
	}

	// Add project filtering at the nanostore level
	if global {
		opts.Filters["_data.Project"] = "global"
	} else if project != "" {
		opts.Filters["_data.Project"] = project
	}

	typedScratches, err := s.store.List(opts)
	if err != nil {
		logger.Error().Err(err).Str("query", query).Msg("Failed to search scratches")
		return []Scratch{}
	}

	scratches := make([]Scratch, len(typedScratches))
	for i, ts := range typedScratches {
		scratches[i] = ts.ToScratch()
	}

	logger.Debug().Int("count", len(scratches)).Str("query", query).Str("project", project).Bool("global", global).Msg("Search completed")
	return scratches
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

// GetTestStore returns the underlying nanostore as TestStore for testing purposes
func (s *Store) GetTestStore() (nanostore.TestStore, bool) {
	if testStore, ok := s.store.Store().(nanostore.TestStore); ok {
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
	// Use TypedStore Get method which accepts both UUIDs and SimpleIDs
	typedScratch, err := s.store.Get(uuid)
	if err != nil {
		return nil, fmt.Errorf("scratch not found: %s", uuid)
	}

	scratch := typedScratch.ToScratch()
	return &scratch, nil
}

// UpdateWhere updates documents matching a custom WHERE clause using nanostore bulk operations
func (s *Store) UpdateWhere(whereClause string, scratch *TypedScratch, args ...interface{}) (int, error) {
	return s.store.UpdateWhere(whereClause, scratch, args...)
}

// Update updates a document by UUID using TypedStore's Update method
func (s *Store) Update(id string, scratch *TypedScratch) error {
	return s.store.Update(id, scratch)
}

// UpdateByUUIDs updates multiple documents by their UUIDs in a single atomic operation
func (s *Store) UpdateByUUIDs(uuids []string, scratch *TypedScratch) (int, error) {
	return s.store.UpdateByUUIDs(uuids, scratch)
}

// DeleteByUUIDs deletes multiple documents by their UUIDs in a single atomic operation
func (s *Store) DeleteByUUIDs(uuids []string) (int, error) {
	return s.store.DeleteByUUIDs(uuids)
}

// ResolveBulkIDs resolves multiple IDs (SimpleIDs, UUIDs, or prefixes) to scratches in a single operation
// This consolidates all ID resolution logic that was previously spread across commands
func (s *Store) ResolveBulkIDs(ids []string, project string, global bool) ([]*Scratch, error) {
	if len(ids) == 0 {
		return []*Scratch{}, nil
	}

	// Remove duplicates while preserving order
	seen := make(map[string]bool)
	uniqueIDs := make([]string, 0, len(ids))
	for _, id := range ids {
		if id != "" && !seen[id] {
			seen[id] = true
			uniqueIDs = append(uniqueIDs, id)
		}
	}

	results := make([]*Scratch, 0, len(uniqueIDs))
	errors := make([]string, 0)

	for _, id := range uniqueIDs {
		scratch, err := s.resolveSingleID(id, project, global)
		if err != nil {
			errors = append(errors, fmt.Sprintf("%s: %v", id, err))
			continue
		}
		results = append(results, scratch)
	}

	if len(errors) > 0 {
		return nil, fmt.Errorf("failed to resolve IDs: %s", strings.Join(errors, "; "))
	}

	return results, nil
}

// resolveSingleID handles all ID resolution patterns consolidated from command layer
func (s *Store) resolveSingleID(id, project string, global bool) (*Scratch, error) {
	// Handle deleted index (d1, d2, etc)
	if len(id) > 1 && id[0] == 'd' {
		return s.resolveDeletedIndex(id, project, global)
	}

	// Try nanostore's built-in resolution first (handles SimpleIDs and UUIDs)
	uuid, err := s.ResolveIDToUUID(id)
	if err == nil {
		return s.getScratchByUUIDWithScope(uuid, project, global)
	}

	// Handle partial UUID prefix matching
	return s.resolvePrefixMatch(id, project, global)
}

// resolveDeletedIndex handles deleted indices (d1, d2, etc)
func (s *Store) resolveDeletedIndex(id, project string, global bool) (*Scratch, error) {
	indexStr := id[1:] // Remove 'd' prefix
	deletedIndex, err := s.parsePositiveInt(indexStr)
	if err != nil {
		return nil, fmt.Errorf("invalid deleted index: %s", id)
	}

	var deletedScratches []Scratch
	if global {
		deletedScratches = s.GetDeletedScratchesWithFilter("", true)
	} else {
		deletedScratches = s.GetDeletedScratchesWithFilter(project, false)
	}

	if deletedIndex < 1 || deletedIndex > len(deletedScratches) {
		return nil, fmt.Errorf("deleted index out of range: %s", id)
	}

	return &deletedScratches[deletedIndex-1], nil
}

// getScratchByUUIDWithScope gets scratch by UUID and validates scope
func (s *Store) getScratchByUUIDWithScope(uuid, project string, global bool) (*Scratch, error) {
	scratch, err := s.GetScratchByUUID(uuid)
	if err != nil {
		return nil, err
	}

	// Verify scope
	if global && scratch.Project != "global" {
		return nil, fmt.Errorf("scratch not found in global scope")
	}
	if !global && project != "" && scratch.Project != project {
		return nil, fmt.Errorf("scratch not found in project scope")
	}

	return scratch, nil
}

// resolvePrefixMatch handles partial UUID prefix matching
func (s *Store) resolvePrefixMatch(id, project string, global bool) (*Scratch, error) {
	var scratches []Scratch
	if global {
		scratches = s.GetAllScratchesWithFilter("", true)
	} else {
		scratches = s.GetAllScratchesWithFilter(project, false)
	}

	for i := range scratches {
		if strings.HasPrefix(scratches[i].ID, id) {
			return &scratches[i], nil
		}
	}

	return nil, fmt.Errorf("scratch not found: %s", id)
}

// parsePositiveInt parses a string as a positive integer with bounds checking
func (s *Store) parsePositiveInt(str string) (int, error) {
	if str == "" {
		return 0, fmt.Errorf("empty string")
	}

	result := 0
	for _, c := range str {
		if c < '0' || c > '9' {
			return 0, fmt.Errorf("non-numeric character")
		}
		result = result*10 + int(c-'0')
		if result > 1000000 { // Reasonable upper bound
			return 0, fmt.Errorf("number too large")
		}
	}

	if result < 1 {
		return 0, fmt.Errorf("must be positive")
	}

	return result, nil
}
