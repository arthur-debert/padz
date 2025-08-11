package store

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
	"sync"
	"time"

	"github.com/adrg/xdg"
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filelock"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/arthur-debert/padz/pkg/logging"
)

const (
	dataDirName      = "scratch"
	metadataFileName = "metadata.json"
)

type Store struct {
	mu              sync.Mutex
	scratches       []Scratch
	fs              filesystem.FileSystem
	cfg             *config.Config
	metadataManager *MetadataManager
	useNewMetadata  bool // Flag to enable new metadata system
}

func NewStore() (*Store, error) {
	logger := logging.GetLogger("store")
	logger.Info().Msg("Initializing new store")
	return NewStoreWithConfig(config.GetConfig())
}

func NewStoreWithConfig(cfg *config.Config) (*Store, error) {
	logger := logging.GetLogger("store")
	logger.Info().Msg("Initializing store with config")

	store := &Store{
		fs:  cfg.FileSystem,
		cfg: cfg,
	}

	// Initialize metadata manager
	basePath, err := GetScratchPathWithConfig(cfg)
	if err != nil {
		return nil, err
	}
	store.metadataManager = NewMetadataManager(cfg.FileSystem, basePath)

	// Check if we should use new metadata system
	store.useNewMetadata = store.shouldUseNewMetadata()

	if err := store.load(); err != nil {
		logger.Error().Err(err).Msg("Failed to load store data")
		return nil, err
	}

	logger.Info().Int("scratch_count", len(store.scratches)).Msg("Store initialized successfully")
	return store, nil
}

func (s *Store) GetScratches() []Scratch {
	s.mu.Lock()
	defer s.mu.Unlock()

	logger := logging.GetLogger("store")
	logger.Debug().Int("scratch_count", len(s.scratches)).Msg("Retrieved all scratches")
	return s.scratches
}

func (s *Store) SaveScratches(scratches []Scratch) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	logger := logging.GetLogger("store")
	oldCount := len(s.scratches)
	newCount := len(scratches)

	logger.Info().Int("old_count", oldCount).Int("new_count", newCount).Msg("Bulk replacing scratches")

	if s.useNewMetadata {
		// Rebuild entire metadata system with new scratches
		// This is expensive but ensures consistency
		logger.Info().Msg("Rebuilding metadata for bulk save")

		// Create new index
		index := &Index{
			Version:   indexVersion,
			UpdatedAt: time.Now(),
			Scratches: make(map[string]IndexEntry),
		}

		// Save all individual metadata files
		for _, scratch := range scratches {
			if err := s.metadataManager.SaveScratchMetadata(&scratch); err != nil {
				logger.Error().Err(err).Str("scratch_id", scratch.ID).Msg("Failed to save scratch metadata during bulk save")
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
		if err := s.metadataManager.SaveIndex(index); err != nil {
			logger.Error().Err(err).Msg("Failed to save index during bulk save")
			return err
		}

		// Clean up orphaned metadata files
		metadataPath := s.metadataManager.GetMetadataPath()
		if entries, err := s.fs.ReadDir(metadataPath); err == nil {
			for _, entry := range entries {
				if strings.HasSuffix(entry.Name(), ".json") {
					id := strings.TrimSuffix(entry.Name(), ".json")
					if _, exists := index.Scratches[id]; !exists {
						// Remove orphaned metadata file
						_ = s.metadataManager.DeleteScratchMetadata(id)
					}
				}
			}
		}
	}

	s.scratches = scratches

	if !s.useNewMetadata {
		if err := s.save(); err != nil {
			logger.Error().Err(err).Int("scratch_count", newCount).Msg("Failed to save scratches after bulk replace")
			return err
		}
	}

	logger.Info().Int("scratch_count", newCount).Msg("Bulk scratches saved successfully")
	return nil
}

// shouldUseNewMetadata checks if we should use the new metadata system
func (s *Store) shouldUseNewMetadata() bool {
	indexPath := s.metadataManager.GetIndexPath()
	if _, err := s.fs.Stat(indexPath); err == nil {
		// Index exists, use new system
		return true
	}

	// Check if metadata directory exists with files
	metadataPath := s.metadataManager.GetMetadataPath()
	if entries, err := s.fs.ReadDir(metadataPath); err == nil && len(entries) > 0 {
		// Metadata directory has files, use new system
		return true
	}

	return false
}

func (s *Store) load() error {
	s.mu.Lock()
	defer s.mu.Unlock()

	logger := logging.GetLogger("store")

	if s.useNewMetadata {
		return s.loadWithNewMetadata()
	}

	path, err := s.getMetadataPathWithStore()
	if err != nil {
		logger.Error().Err(err).Msg("Failed to get metadata path during load")
		return err
	}

	logger.Debug().Str("path", path).Msg("Loading store data from metadata file")

	if _, err := s.fs.Stat(path); os.IsNotExist(err) {
		logger.Info().Str("path", path).Msg("Metadata file does not exist, initializing empty store")
		s.scratches = []Scratch{}
		return nil
	}

	data, err := s.fs.ReadFile(path)
	if err != nil {
		logger.Error().Err(err).Str("path", path).Msg("Failed to read metadata file")
		return err
	}

	logger.Debug().Int("bytes_read", len(data)).Str("path", path).Msg("Successfully read metadata file")

	if err := json.Unmarshal(data, &s.scratches); err != nil {
		logger.Error().Err(err).Str("path", path).Int("bytes_read", len(data)).Msg("Failed to unmarshal JSON data")
		return err
	}

	logger.Info().Int("scratch_count", len(s.scratches)).Str("path", path).Msg("Store data loaded successfully")

	// Check if we should migrate to new system
	if !s.useNewMetadata && len(s.scratches) > 0 {
		logger.Info().Msg("Legacy metadata detected, migrating to new system")
		if err := s.metadataManager.MigrateFromLegacyMetadata(s.scratches); err != nil {
			logger.Error().Err(err).Msg("Failed to migrate to new metadata system")
			// Continue with old system if migration fails
		} else {
			s.useNewMetadata = true
			logger.Info().Msg("Migration to new metadata system completed")
		}
	}

	return nil
}

// loadWithNewMetadata loads scratches using the new metadata system
func (s *Store) loadWithNewMetadata() error {
	logger := logging.GetLogger("store")
	logger.Debug().Msg("Loading store data using new metadata system")

	// Initialize metadata system if needed
	if err := s.metadataManager.Initialize(); err != nil {
		return err
	}

	// Load index
	index, err := s.metadataManager.LoadIndex()
	if err != nil {
		return err
	}

	// Load individual metadata for each scratch in index
	s.scratches = make([]Scratch, 0, len(index.Scratches))
	for id, entry := range index.Scratches {
		scratch, err := s.metadataManager.LoadScratchMetadata(id)
		if err != nil {
			logger.Warn().Err(err).Str("id", id).Msg("Failed to load scratch metadata")
			continue
		}
		if scratch == nil {
			// Metadata file missing, reconstruct from index
			scratch = &Scratch{
				ID:        id,
				Project:   entry.Project,
				Title:     entry.Title,
				CreatedAt: entry.CreatedAt,
				UpdatedAt: entry.CreatedAt, // Best guess - use creation time
				Size:      0,               // Unknown without reading file
				Checksum:  "",              // Unknown without reading file
			}
			logger.Warn().
				Str("id", id).
				Str("project", entry.Project).
				Msg("Reconstructed scratch from index - metadata file missing")
		}
		s.scratches = append(s.scratches, *scratch)
	}

	logger.Info().Int("scratch_count", len(s.scratches)).Msg("Store data loaded successfully with new metadata system")
	return nil
}

func (s *Store) save() error {
	logger := logging.GetLogger("store")

	if s.useNewMetadata {
		return s.saveWithNewMetadata()
	}

	path, err := s.getMetadataPathWithStore()
	if err != nil {
		logger.Error().Err(err).Msg("Failed to get metadata path during save")
		return err
	}

	logger.Debug().Str("path", path).Int("scratch_count", len(s.scratches)).Msg("Saving store data to metadata file")

	data, err := json.MarshalIndent(s.scratches, "", "  ")
	if err != nil {
		logger.Error().Err(err).Int("scratch_count", len(s.scratches)).Msg("Failed to marshal scratches to JSON")
		return err
	}

	logger.Debug().Int("bytes_to_write", len(data)).Str("path", path).Msg("Successfully marshaled data, writing to file")

	if err := s.fs.WriteFile(path, data, 0644); err != nil {
		logger.Error().Err(err).Str("path", path).Int("bytes_to_write", len(data)).Msg("Failed to write metadata file")
		return err
	}

	logger.Debug().Str("path", path).Int("scratch_count", len(s.scratches)).Msg("Store data saved successfully")
	return nil
}

// saveWithNewMetadata saves using the new metadata system
func (s *Store) saveWithNewMetadata() error {
	logger := logging.GetLogger("store")
	logger.Debug().Msg("Saving store data using new metadata system")

	// This is now a no-op as individual operations update metadata
	// The index is updated with each operation
	logger.Debug().Msg("Save called but using new metadata system (no-op)")
	return nil
}

func (s *Store) AddScratch(scratch Scratch) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	logger := logging.GetLogger("store")
	logger.Info().Str("scratch_id", scratch.ID).Str("title", scratch.Title).Msg("Adding new scratch")

	if s.useNewMetadata {
		// Save individual metadata file
		if err := s.metadataManager.SaveScratchMetadata(&scratch); err != nil {
			logger.Error().Err(err).Str("scratch_id", scratch.ID).Msg("Failed to save scratch metadata")
			return err
		}

		// Update index
		if err := s.metadataManager.UpdateIndexEntry(&scratch); err != nil {
			logger.Error().Err(err).Str("scratch_id", scratch.ID).Msg("Failed to update index")
			return err
		}
	}

	s.scratches = append(s.scratches, scratch)

	if !s.useNewMetadata {
		if err := s.save(); err != nil {
			logger.Error().Err(err).Str("scratch_id", scratch.ID).Msg("Failed to save after adding scratch")
			return err
		}
	}

	logger.Info().Str("scratch_id", scratch.ID).Int("total_count", len(s.scratches)).Msg("Scratch added successfully")
	return nil
}

func (s *Store) RemoveScratch(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	logger := logging.GetLogger("store")
	oldCount := len(s.scratches)
	logger.Info().Str("scratch_id", id).Int("current_count", oldCount).Msg("Removing scratch")

	if s.useNewMetadata {
		// Delete individual metadata file
		if err := s.metadataManager.DeleteScratchMetadata(id); err != nil {
			logger.Error().Err(err).Str("scratch_id", id).Msg("Failed to delete scratch metadata")
			return err
		}

		// Remove from index
		if err := s.metadataManager.RemoveIndexEntry(id); err != nil {
			logger.Error().Err(err).Str("scratch_id", id).Msg("Failed to update index")
			return err
		}
	}

	var newScratches []Scratch
	found := false
	for _, scratch := range s.scratches {
		if scratch.ID != id {
			newScratches = append(newScratches, scratch)
		} else {
			found = true
			logger.Debug().Str("scratch_id", id).Str("title", scratch.Title).Msg("Found scratch to remove")
		}
	}

	if !found {
		logger.Warn().Str("scratch_id", id).Msg("Scratch not found for removal")
	}

	s.scratches = newScratches
	newCount := len(s.scratches)

	if !s.useNewMetadata {
		if err := s.save(); err != nil {
			logger.Error().Err(err).Str("scratch_id", id).Msg("Failed to save after removing scratch")
			return err
		}
	}

	logger.Info().Str("scratch_id", id).Int("old_count", oldCount).Int("new_count", newCount).Bool("found", found).Msg("Scratch removal completed")
	return nil
}

func (s *Store) UpdateScratch(scratchToUpdate Scratch) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	logger := logging.GetLogger("store")
	logger.Info().Str("scratch_id", scratchToUpdate.ID).Str("title", scratchToUpdate.Title).Msg("Updating scratch")

	if s.useNewMetadata {
		// Update timestamp
		scratchToUpdate.UpdatedAt = time.Now()

		// Save individual metadata file
		if err := s.metadataManager.SaveScratchMetadata(&scratchToUpdate); err != nil {
			logger.Error().Err(err).Str("scratch_id", scratchToUpdate.ID).Msg("Failed to save scratch metadata")
			return err
		}

		// Update index
		if err := s.metadataManager.UpdateIndexEntry(&scratchToUpdate); err != nil {
			logger.Error().Err(err).Str("scratch_id", scratchToUpdate.ID).Msg("Failed to update index")
			return err
		}
	}

	found := false
	for i, scratch := range s.scratches {
		if scratch.ID == scratchToUpdate.ID {
			logger.Debug().Str("scratch_id", scratchToUpdate.ID).Str("old_title", scratch.Title).Str("new_title", scratchToUpdate.Title).Msg("Found scratch to update")
			s.scratches[i] = scratchToUpdate
			found = true
			break
		}
	}

	if !found {
		logger.Warn().Str("scratch_id", scratchToUpdate.ID).Msg("Scratch not found for update")
	}

	if !s.useNewMetadata {
		if err := s.save(); err != nil {
			logger.Error().Err(err).Str("scratch_id", scratchToUpdate.ID).Msg("Failed to save after updating scratch")
			return err
		}
	}

	logger.Info().Str("scratch_id", scratchToUpdate.ID).Bool("found", found).Msg("Scratch update completed")
	return nil
}

func GetScratchPath() (string, error) {
	cfg := config.GetConfig()
	return GetScratchPathWithConfig(cfg)
}

func GetScratchPathWithConfig(cfg *config.Config) (string, error) {
	logger := logging.GetLogger("store.path")
	var path string
	var err error

	if cfg.DataPath != "" {
		// Use configured path for testing
		path = cfg.FileSystem.Join(cfg.DataPath, dataDirName)
		logger.Debug().Str("path", path).Msg("Using configured data path")
	} else {
		// Use XDG for production
		path, err = xdg.DataFile(dataDirName)
		if err != nil {
			logger.Error().Err(err).Msg("Failed to get XDG data file path")
			return "", err
		}
		logger.Debug().Str("path", path).Msg("Using XDG data path")
	}

	logger.Debug().Str("path", path).Msg("Creating scratch directory if needed")
	if err := cfg.FileSystem.MkdirAll(path, 0755); err != nil {
		logger.Error().Err(err).Str("path", path).Msg("Failed to create scratch directory")
		return "", err
	}

	logger.Debug().Str("path", path).Msg("Scratch path resolved successfully")
	return path, nil
}

func GetScratchFilePath(id string) (string, error) {
	cfg := config.GetConfig()
	return GetScratchFilePathWithConfig(id, cfg)
}

func GetScratchFilePathWithConfig(id string, cfg *config.Config) (string, error) {
	logger := logging.GetLogger("store.path")

	path, err := GetScratchPathWithConfig(cfg)
	if err != nil {
		logger.Error().Err(err).Str("scratch_id", id).Msg("Failed to get base scratch path")
		return "", err
	}

	// Check if new metadata system is in use
	metadataManager := NewMetadataManager(cfg.FileSystem, path)
	filesPath := metadataManager.GetFilesPath()
	if _, err := cfg.FileSystem.Stat(filesPath); err == nil {
		// New structure exists, use it
		filePath := cfg.FileSystem.Join(filesPath, id)
		logger.Debug().Str("scratch_id", id).Str("file_path", filePath).Msg("Resolved scratch file path (new structure)")
		return filePath, nil
	}

	// Fall back to old structure
	filePath := cfg.FileSystem.Join(path, id)
	logger.Debug().Str("scratch_id", id).Str("file_path", filePath).Msg("Resolved scratch file path (legacy structure)")
	return filePath, nil
}

func getMetadataPathWithConfig(cfg *config.Config) (string, error) {
	logger := logging.GetLogger("store.path")

	path, err := GetScratchPathWithConfig(cfg)
	if err != nil {
		logger.Error().Err(err).Msg("Failed to get base scratch path for metadata")
		return "", err
	}

	metadataPath := cfg.FileSystem.Join(path, metadataFileName)
	logger.Debug().Str("metadata_path", metadataPath).Msg("Resolved metadata file path")
	return metadataPath, nil
}

func (s *Store) getMetadataPathWithStore() (string, error) {
	return getMetadataPathWithConfig(s.cfg)
}

// RebuildIndex rebuilds the master index from individual metadata files
func (s *Store) RebuildIndex() error {
	if !s.useNewMetadata {
		return fmt.Errorf("new metadata system not in use")
	}

	return s.metadataManager.RebuildIndex()
}

// withFileLock performs an operation with file-based locking to prevent concurrent metadata corruption
func (s *Store) withFileLock(operation func() error) error {
	logger := logging.GetLogger("store")

	var lockPath string
	var err error

	if s.useNewMetadata {
		// Lock on index file for new system
		lockPath = s.metadataManager.GetIndexPath()
	} else {
		// Lock on metadata.json for old system
		lockPath, err = s.getMetadataPathWithStore()
		if err != nil {
			return err
		}
	}

	lock := filelock.New(lockPath)

	// Try to acquire lock with 5 second timeout
	if err := lock.Lock(5 * time.Second); err != nil {
		logger.Error().Err(err).Msg("Failed to acquire file lock")
		return err
	}
	defer func() {
		if unlockErr := lock.Unlock(); unlockErr != nil {
			logger.Error().Err(unlockErr).Msg("Failed to release file lock")
		}
	}()

	// Reload the latest data while holding the lock
	if err := s.load(); err != nil {
		return err
	}

	// Perform the operation
	if err := operation(); err != nil {
		return err
	}

	// Save the changes
	return s.save()
}

// AddScratchAtomic adds a scratch with file locking to prevent concurrent conflicts
func (s *Store) AddScratchAtomic(scratch Scratch) error {
	// If using memory filesystem (for tests), fall back to non-atomic
	if _, ok := s.fs.(*filesystem.MemoryFileSystem); ok {
		return s.AddScratch(scratch)
	}

	return s.withFileLock(func() error {
		logger := logging.GetLogger("store")

		// Check for duplicates
		for _, existing := range s.scratches {
			if existing.ID == scratch.ID {
				logger.Warn().Str("scratch_id", scratch.ID).Msg("Scratch with this ID already exists, skipping add")
				return nil
			}
		}

		logger.Info().Str("scratch_id", scratch.ID).Str("title", scratch.Title).Msg("Adding new scratch atomically")

		if s.useNewMetadata {
			// Save individual metadata file (no lock needed for individual file)
			if err := s.metadataManager.SaveScratchMetadata(&scratch); err != nil {
				return err
			}

			// Update index (already within lock)
			if err := s.metadataManager.UpdateIndexEntry(&scratch); err != nil {
				return err
			}
		}

		s.scratches = append(s.scratches, scratch)
		return nil
	})
}

// RemoveScratchAtomic removes a scratch with file locking
func (s *Store) RemoveScratchAtomic(id string) error {
	// If using memory filesystem (for tests), fall back to non-atomic
	if _, ok := s.fs.(*filesystem.MemoryFileSystem); ok {
		return s.RemoveScratch(id)
	}

	return s.withFileLock(func() error {
		logger := logging.GetLogger("store")

		var newScratches []Scratch
		found := false

		for _, scratch := range s.scratches {
			if scratch.ID != id {
				newScratches = append(newScratches, scratch)
			} else {
				found = true
				logger.Debug().Str("scratch_id", id).Str("title", scratch.Title).Msg("Found scratch to remove")
			}
		}

		if !found {
			logger.Warn().Str("scratch_id", id).Msg("Scratch not found for removal")
		}

		if s.useNewMetadata && found {
			// Delete individual metadata file
			if err := s.metadataManager.DeleteScratchMetadata(id); err != nil {
				return err
			}

			// Remove from index
			if err := s.metadataManager.RemoveIndexEntry(id); err != nil {
				return err
			}
		}

		s.scratches = newScratches
		return nil
	})
}

// UpdateScratchAtomic updates a scratch with file locking
func (s *Store) UpdateScratchAtomic(scratchToUpdate Scratch) error {
	// If using memory filesystem (for tests), fall back to non-atomic
	if _, ok := s.fs.(*filesystem.MemoryFileSystem); ok {
		return s.UpdateScratch(scratchToUpdate)
	}

	return s.withFileLock(func() error {
		logger := logging.GetLogger("store")

		found := false
		for i, scratch := range s.scratches {
			if scratch.ID == scratchToUpdate.ID {
				logger.Debug().Str("scratch_id", scratchToUpdate.ID).Msg("Found scratch to update")
				s.scratches[i] = scratchToUpdate
				found = true
				break
			}
		}

		if !found {
			logger.Warn().Str("scratch_id", scratchToUpdate.ID).Msg("Scratch not found for update")
		}

		if s.useNewMetadata && found {
			// Update timestamp
			scratchToUpdate.UpdatedAt = time.Now()

			// Save individual metadata file
			if err := s.metadataManager.SaveScratchMetadata(&scratchToUpdate); err != nil {
				return err
			}

			// Update index
			if err := s.metadataManager.UpdateIndexEntry(&scratchToUpdate); err != nil {
				return err
			}
		}

		return nil
	})
}

// SaveScratchesAtomic performs bulk update with file locking
func (s *Store) SaveScratchesAtomic(scratches []Scratch) error {
	// If using memory filesystem (for tests), fall back to non-atomic
	if _, ok := s.fs.(*filesystem.MemoryFileSystem); ok {
		return s.SaveScratches(scratches)
	}

	return s.withFileLock(func() error {
		logger := logging.GetLogger("store")
		logger.Info().Int("old_count", len(s.scratches)).Int("new_count", len(scratches)).Msg("Bulk replacing scratches atomically")

		if s.useNewMetadata {
			// Rebuild metadata within lock
			// Create new index
			index := &Index{
				Version:   indexVersion,
				UpdatedAt: time.Now(),
				Scratches: make(map[string]IndexEntry),
			}

			// Save all individual metadata files
			for _, scratch := range scratches {
				if err := s.metadataManager.SaveScratchMetadata(&scratch); err != nil {
					logger.Error().Err(err).Str("scratch_id", scratch.ID).Msg("Failed to save scratch metadata during bulk save")
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
			if err := s.metadataManager.SaveIndex(index); err != nil {
				return err
			}
		}

		s.scratches = scratches
		return nil
	})
}
