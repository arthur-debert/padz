package v2

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/arthur-debert/nanostore/nanostore/api"
)

const storeFileName = "padz-scratches.json"

// SimpleStore is a minimal nanostore implementation for testing
type SimpleStore struct {
	store     *api.TypedStore[PadzScratch]
	basePath  string
	filesPath string
}

// NewSimpleStore creates a new simple store for testing
func NewSimpleStore(basePath string) (*SimpleStore, error) {
	// Ensure base directory exists
	if err := os.MkdirAll(basePath, 0755); err != nil {
		return nil, fmt.Errorf("failed to create base directory: %w", err)
	}

	// Create files subdirectory
	filesPath := filepath.Join(basePath, "files")
	if err := os.MkdirAll(filesPath, 0755); err != nil {
		return nil, fmt.Errorf("failed to create files directory: %w", err)
	}

	storePath := filepath.Join(basePath, storeFileName)

	// Initialize nanostore
	typedStore, err := api.NewFromType[PadzScratch](storePath)
	if err != nil {
		return nil, fmt.Errorf("failed to initialize nanostore: %w", err)
	}

	return &SimpleStore{
		store:     typedStore,
		basePath:  basePath,
		filesPath: filesPath,
	}, nil
}

// GetScratches returns all active scratches
func (s *SimpleStore) GetScratches() ([]Scratch, error) {
	results, err := s.store.Query().
		Activity("active").
		Find()
	if err != nil {
		return nil, err
	}

	scratches := make([]Scratch, len(results))
	for i, ps := range results {
		scratch := ps.ToScratch()
		// Use SimpleID if available, otherwise keep UUID
		if ps.SimpleID != "" {
			scratch.ID = ps.SimpleID
		}
		scratches[i] = scratch
	}

	return scratches, nil
}

// GetPinnedScratches returns only pinned scratches
func (s *SimpleStore) GetPinnedScratches() ([]Scratch, error) {
	// First get all active items
	results, err := s.store.Query().
		Activity("active").
		Find()
	if err != nil {
		return nil, err
	}

	// Filter for pinned items
	var pinned []PadzScratch
	for _, ps := range results {
		if ps.Pinned == "yes" {
			pinned = append(pinned, ps)
		}
	}

	// Sort by PinnedAt descending
	// TODO: Add proper sorting when supported by API

	scratches := make([]Scratch, len(pinned))
	for i, ps := range pinned {
		scratch := ps.ToScratch()
		// Use SimpleID if available, otherwise keep UUID
		if ps.SimpleID != "" {
			scratch.ID = ps.SimpleID
		}
		scratches[i] = scratch
	}

	return scratches, nil
}

// AddScratch adds a new scratch
func (s *SimpleStore) AddScratch(scratch Scratch) (string, error) {
	ps := FromScratch(scratch)

	// Save content to file if present
	if scratch.ID != "" && len(ps.Content) > 0 {
		contentPath := filepath.Join(s.filesPath, scratch.ID)
		if err := os.WriteFile(contentPath, []byte(ps.Content), 0644); err != nil {
			return "", fmt.Errorf("failed to save content: %w", err)
		}
		ps.Content = "" // Don't store content in nanostore
	}

	id, err := s.store.Create(scratch.Title, ps)
	if err != nil {
		return "", fmt.Errorf("failed to create scratch: %w", err)
	}

	return id, nil
}

// RemoveScratch soft deletes a scratch
func (s *SimpleStore) RemoveScratch(id string) error {
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
	ps.Activity = "deleted"
	ps.DeletedAt = time.Now()

	if err := s.store.Update(uuid, ps); err != nil {
		return fmt.Errorf("failed to update scratch: %w", err)
	}

	return nil
}

// UpdateScratch updates an existing scratch
func (s *SimpleStore) UpdateScratch(scratch Scratch) error {
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
		existing.Activity = "deleted"
		if scratch.DeletedAt != nil {
			existing.DeletedAt = *scratch.DeletedAt
		}
	} else {
		existing.Activity = "active"
		existing.DeletedAt = time.Time{}
	}

	if err := s.store.Update(uuid, existing); err != nil {
		return fmt.Errorf("failed to update scratch: %w", err)
	}

	return nil
}

// Close closes the store
func (s *SimpleStore) Close() error {
	return s.store.Close()
}

// resolveID converts user-facing IDs to UUIDs
func (s *SimpleStore) resolveID(id string) (string, error) {
	// Get all items (both active and deleted)
	// First try just Activity=active since that's the default canonical view
	allResults, err := s.store.Query().Activity("active").Find()
	if err != nil {
		// If default query fails, try explicit queries
		activeResults, err1 := s.store.Query().Activity("active").Find()
		if err1 != nil {
			return "", err1
		}

		deletedResults, err2 := s.store.Query().Activity("deleted").Find()
		if err2 == nil {
			allResults = append(activeResults, deletedResults...)
		} else {
			allResults = activeResults
		}
	}

	// Check if it's a simple numeric ID or pinned ID
	for _, ps := range allResults {
		if ps.SimpleID == id {
			return ps.UUID, nil
		}
	}

	// Check if it's a UUID or partial UUID
	for _, ps := range allResults {
		if ps.UUID == id || (len(id) >= 8 && strings.HasPrefix(ps.UUID, id)) {
			return ps.UUID, nil
		}
	}

	return "", fmt.Errorf("scratch not found: %s", id)
}
