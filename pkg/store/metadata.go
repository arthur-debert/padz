package store

import (
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/arthur-debert/padz/pkg/logging"
)

const (
	metadataDirName = "metadata"
	filesDirName    = "files"
	indexFileName   = "index.json"
	indexVersion    = "1.0"
)

// MetadataManager handles individual metadata files and master index
type MetadataManager struct {
	fs       filesystem.FileSystem
	basePath string
}

// NewMetadataManager creates a new metadata manager
func NewMetadataManager(fs filesystem.FileSystem, basePath string) *MetadataManager {
	return &MetadataManager{
		fs:       fs,
		basePath: basePath,
	}
}

// GetFilesPath returns the path to the files directory
func (m *MetadataManager) GetFilesPath() string {
	return m.fs.Join(m.basePath, filesDirName)
}

// GetMetadataPath returns the path to the metadata directory
func (m *MetadataManager) GetMetadataPath() string {
	return m.fs.Join(m.basePath, metadataDirName)
}

// GetIndexPath returns the path to the index file
func (m *MetadataManager) GetIndexPath() string {
	return m.fs.Join(m.basePath, indexFileName)
}

// Initialize creates the necessary directory structure
func (m *MetadataManager) Initialize() error {
	logger := logging.GetLogger("metadata")

	// Create files directory
	filesPath := m.GetFilesPath()
	if err := m.fs.MkdirAll(filesPath, 0755); err != nil {
		logger.Error().Err(err).Str("path", filesPath).Msg("Failed to create files directory")
		return err
	}

	// Create metadata directory
	metadataPath := m.GetMetadataPath()
	if err := m.fs.MkdirAll(metadataPath, 0755); err != nil {
		logger.Error().Err(err).Str("path", metadataPath).Msg("Failed to create metadata directory")
		return err
	}

	// Check if index exists, if not create empty one
	indexPath := m.GetIndexPath()
	if _, err := m.fs.Stat(indexPath); os.IsNotExist(err) {
		index := &Index{
			Version:   indexVersion,
			UpdatedAt: time.Now(),
			Scratches: make(map[string]IndexEntry),
		}
		if err := m.SaveIndex(index); err != nil {
			return err
		}
	}

	return nil
}

// LoadIndex reads the master index
func (m *MetadataManager) LoadIndex() (*Index, error) {
	indexPath := m.GetIndexPath()

	data, err := m.fs.ReadFile(indexPath)
	if err != nil {
		if os.IsNotExist(err) {
			// Return empty index if file doesn't exist
			return &Index{
				Version:   indexVersion,
				UpdatedAt: time.Now(),
				Scratches: make(map[string]IndexEntry),
			}, nil
		}
		logger := logging.GetLogger("metadata")
		logger.Error().Err(err).Str("path", indexPath).Msg("Failed to read index file")
		return nil, err
	}

	var index Index
	if err := json.Unmarshal(data, &index); err != nil {
		logger := logging.GetLogger("metadata")
		logger.Error().Err(err).Msg("Failed to unmarshal index")
		return nil, err
	}

	return &index, nil
}

// SaveIndex writes the master index
func (m *MetadataManager) SaveIndex(index *Index) error {
	index.UpdatedAt = time.Now()

	logger := logging.GetLogger("metadata")

	data, err := json.MarshalIndent(index, "", "  ")
	if err != nil {
		logger.Error().Err(err).Msg("Failed to marshal index")
		return err
	}

	indexPath := m.GetIndexPath()
	if err := m.fs.WriteFile(indexPath, data, 0644); err != nil {
		logger.Error().Err(err).Str("path", indexPath).Msg("Failed to write index file")
		return err
	}

	logger.Debug().Str("path", indexPath).Int("scratch_count", len(index.Scratches)).Msg("Index saved successfully")
	return nil
}

// LoadScratchMetadata loads individual scratch metadata
func (m *MetadataManager) LoadScratchMetadata(id string) (*Scratch, error) {
	metadataPath := m.fs.Join(m.GetMetadataPath(), id+".json")

	logger := logging.GetLogger("metadata")

	data, err := m.fs.ReadFile(metadataPath)
	if err != nil {
		if os.IsNotExist(err) {
			logger.Debug().Str("id", id).Msg("Metadata file not found")
			return nil, nil
		}
		logger.Error().Err(err).Str("path", metadataPath).Msg("Failed to read metadata file")
		return nil, err
	}

	var scratch Scratch
	if err := json.Unmarshal(data, &scratch); err != nil {
		logger.Error().Err(err).Str("id", id).Msg("Failed to unmarshal scratch metadata")
		return nil, err
	}

	return &scratch, nil
}

// SaveScratchMetadata saves individual scratch metadata
func (m *MetadataManager) SaveScratchMetadata(scratch *Scratch) error {
	// Update timestamps
	if scratch.UpdatedAt.IsZero() {
		scratch.UpdatedAt = time.Now()
	}

	// Calculate checksum if not set
	if scratch.Checksum == "" {
		contentPath := m.fs.Join(m.GetFilesPath(), scratch.ID)
		if data, err := m.fs.ReadFile(contentPath); err == nil {
			scratch.Checksum = fmt.Sprintf("sha256:%x", sha256.Sum256(data))
			scratch.Size = int64(len(data))
		}
	}

	logger := logging.GetLogger("metadata")

	data, err := json.MarshalIndent(scratch, "", "  ")
	if err != nil {
		logger.Error().Err(err).Str("id", scratch.ID).Msg("Failed to marshal scratch metadata")
		return err
	}

	metadataPath := m.fs.Join(m.GetMetadataPath(), scratch.ID+".json")
	if err := m.fs.WriteFile(metadataPath, data, 0644); err != nil {
		logger.Error().Err(err).Str("path", metadataPath).Msg("Failed to write metadata file")
		return err
	}

	logger.Debug().Str("id", scratch.ID).Str("path", metadataPath).Msg("Scratch metadata saved")
	return nil
}

// DeleteScratchMetadata removes individual scratch metadata
func (m *MetadataManager) DeleteScratchMetadata(id string) error {
	metadataPath := m.fs.Join(m.GetMetadataPath(), id+".json")

	logger := logging.GetLogger("metadata")

	if err := m.fs.Remove(metadataPath); err != nil && !os.IsNotExist(err) {
		logger.Error().Err(err).Str("path", metadataPath).Msg("Failed to delete metadata file")
		return err
	}

	logger.Debug().Str("id", id).Msg("Scratch metadata deleted")
	return nil
}

// UpdateIndexEntry updates or adds an entry in the index
func (m *MetadataManager) UpdateIndexEntry(scratch *Scratch) error {
	index, err := m.LoadIndex()
	if err != nil {
		return err
	}

	index.Scratches[scratch.ID] = IndexEntry{
		Project:   scratch.Project,
		Title:     scratch.Title,
		CreatedAt: scratch.CreatedAt,
	}

	return m.SaveIndex(index)
}

// RemoveIndexEntry removes an entry from the index
func (m *MetadataManager) RemoveIndexEntry(id string) error {
	index, err := m.LoadIndex()
	if err != nil {
		return err
	}

	delete(index.Scratches, id)

	return m.SaveIndex(index)
}

// RebuildIndex reconstructs the index from individual metadata files
func (m *MetadataManager) RebuildIndex() error {
	logger := logging.GetLogger("metadata")
	logger.Info().Msg("Rebuilding index from individual metadata files")

	metadataPath := m.GetMetadataPath()
	entries, err := m.fs.ReadDir(metadataPath)
	if err != nil {
		logger.Error().Err(err).Str("path", metadataPath).Msg("Failed to read metadata directory")
		return err
	}

	index := &Index{
		Version:   indexVersion,
		UpdatedAt: time.Now(),
		Scratches: make(map[string]IndexEntry),
	}

	for _, entry := range entries {
		if filepath.Ext(entry.Name()) != ".json" {
			continue
		}

		id := entry.Name()[:len(entry.Name())-5] // Remove .json extension
		scratch, err := m.LoadScratchMetadata(id)
		if err != nil {
			logger.Warn().Err(err).Str("id", id).Msg("Failed to load scratch metadata during rebuild")
			continue
		}

		if scratch != nil {
			index.Scratches[id] = IndexEntry{
				Project:   scratch.Project,
				Title:     scratch.Title,
				CreatedAt: scratch.CreatedAt,
			}
		}
	}

	if err := m.SaveIndex(index); err != nil {
		return err
	}

	logger.Info().Int("scratch_count", len(index.Scratches)).Msg("Index rebuilt successfully")
	return nil
}

// MigrateFromLegacyMetadata migrates from old metadata.json to new structure
func (m *MetadataManager) MigrateFromLegacyMetadata(scratches []Scratch) error {
	logger := logging.GetLogger("metadata")
	logger.Info().Int("scratch_count", len(scratches)).Msg("Starting migration from legacy metadata")

	// Initialize directory structure
	if err := m.Initialize(); err != nil {
		return err
	}

	// Create individual metadata files and build index
	index := &Index{
		Version:   indexVersion,
		UpdatedAt: time.Now(),
		Scratches: make(map[string]IndexEntry),
	}

	for _, scratch := range scratches {
		// Move content file if it's in the wrong place
		oldPath := m.fs.Join(m.basePath, scratch.ID)
		newPath := m.fs.Join(m.GetFilesPath(), scratch.ID)

		if _, err := m.fs.Stat(oldPath); err == nil {
			// File exists in old location, move it
			if content, err := m.fs.ReadFile(oldPath); err == nil {
				if err := m.fs.WriteFile(newPath, content, 0644); err != nil {
					logger.Error().Err(err).Str("id", scratch.ID).Msg("Failed to move content file")
					continue
				}
				// Remove old file
				_ = m.fs.Remove(oldPath)
				logger.Debug().Str("id", scratch.ID).Msg("Moved content file to new location")
			}
		}

		// Save individual metadata
		if err := m.SaveScratchMetadata(&scratch); err != nil {
			logger.Error().Err(err).Str("id", scratch.ID).Msg("Failed to save scratch metadata during migration")
			continue
		}

		// Add to index
		index.Scratches[scratch.ID] = IndexEntry{
			Project:   scratch.Project,
			Title:     scratch.Title,
			CreatedAt: scratch.CreatedAt,
		}
	}

	// Save index
	if err := m.SaveIndex(index); err != nil {
		return err
	}

	// Backup old metadata.json
	oldMetadataPath := m.fs.Join(m.basePath, metadataFileName)
	backupPath := m.fs.Join(m.basePath, metadataFileName+".backup")
	if _, err := m.fs.Stat(oldMetadataPath); err == nil {
		if data, err := m.fs.ReadFile(oldMetadataPath); err == nil {
			_ = m.fs.WriteFile(backupPath, data, 0644)
			logger.Info().Str("backup_path", backupPath).Msg("Created backup of old metadata.json")
		}
	}

	logger.Info().Int("migrated_count", len(index.Scratches)).Msg("Migration completed successfully")
	return nil
}
