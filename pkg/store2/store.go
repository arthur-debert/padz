package store2

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/google/uuid"
)

// Pad represents a single piece of stored content
type Pad struct {
	ID        string    `json:"id"`      // UUID for internal use
	UserID    int       `json:"user_id"` // User-friendly integer ID
	Content   string    `json:"-"`       // Content is stored separately
	Title     string    `json:"title"`
	CreatedAt time.Time `json:"created_at"`
	Size      int64     `json:"size"`
	Checksum  string    `json:"checksum"`
}

// Metadata holds the store's metadata
type Metadata struct {
	Version   string          `json:"version"`
	NextID    int             `json:"next_id"` // Next user-friendly ID to assign
	Pads      map[string]*Pad `json:"pads"`    // Keyed by UUID
	UpdatedAt time.Time       `json:"updated_at"`
}

// Store represents a single-scope store implementation
type Store struct {
	path     string // Base directory for this store
	metadata *Metadata
	mu       sync.RWMutex
}

// NewStore creates a new single-scope store
func NewStore(path string) (*Store, error) {
	// Ensure directory exists
	if err := os.MkdirAll(path, 0755); err != nil {
		return nil, fmt.Errorf("failed to create store directory: %w", err)
	}

	// Ensure data subdirectory exists
	dataDir := filepath.Join(path, "data")
	if err := os.MkdirAll(dataDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create data directory: %w", err)
	}

	s := &Store{
		path: path,
	}

	// Load or initialize metadata
	if err := s.loadMetadata(); err != nil {
		return nil, err
	}

	return s, nil
}

// loadMetadata loads metadata from disk or creates new metadata
func (s *Store) loadMetadata() error {
	metaPath := filepath.Join(s.path, "metadata.json")

	data, err := os.ReadFile(metaPath)
	if err != nil {
		if os.IsNotExist(err) {
			// Initialize new metadata
			s.metadata = &Metadata{
				Version:   "2.0",
				NextID:    1,
				Pads:      make(map[string]*Pad),
				UpdatedAt: time.Now(),
			}
			return s.saveMetadata()
		}
		return fmt.Errorf("failed to read metadata: %w", err)
	}

	var meta Metadata
	if err := json.Unmarshal(data, &meta); err != nil {
		return fmt.Errorf("failed to parse metadata: %w", err)
	}

	s.metadata = &meta
	return nil
}

// saveMetadata saves metadata to disk
func (s *Store) saveMetadata() error {
	metaPath := filepath.Join(s.path, "metadata.json")
	s.metadata.UpdatedAt = time.Now()

	data, err := json.MarshalIndent(s.metadata, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to marshal metadata: %w", err)
	}

	// Write atomically
	tmpPath := metaPath + ".tmp"
	if err := os.WriteFile(tmpPath, data, 0644); err != nil {
		return fmt.Errorf("failed to write metadata: %w", err)
	}

	if err := os.Rename(tmpPath, metaPath); err != nil {
		return fmt.Errorf("failed to rename metadata: %w", err)
	}

	return nil
}

// generateID generates a unique UUID for content
func generateID(content string) string {
	return uuid.New().String()
}

// calculateChecksum calculates SHA256 checksum of content
func calculateChecksum(content string) string {
	h := sha256.Sum256([]byte(content))
	return hex.EncodeToString(h[:])
}
