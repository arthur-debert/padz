package store

import (
	"encoding/json"
	"os"
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
	mu        sync.Mutex
	scratches []Scratch
	fs        filesystem.FileSystem
	cfg       *config.Config
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
	s.scratches = scratches

	if err := s.save(); err != nil {
		logger.Error().Err(err).Int("scratch_count", newCount).Msg("Failed to save scratches after bulk replace")
		return err
	}

	logger.Info().Int("scratch_count", newCount).Msg("Bulk scratches saved successfully")
	return nil
}

func (s *Store) load() error {
	s.mu.Lock()
	defer s.mu.Unlock()

	logger := logging.GetLogger("store")

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
	return nil
}

func (s *Store) save() error {
	logger := logging.GetLogger("store")

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

func (s *Store) AddScratch(scratch Scratch) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	logger := logging.GetLogger("store")
	logger.Info().Str("scratch_id", scratch.ID).Str("title", scratch.Title).Msg("Adding new scratch")

	s.scratches = append(s.scratches, scratch)

	if err := s.save(); err != nil {
		logger.Error().Err(err).Str("scratch_id", scratch.ID).Msg("Failed to save after adding scratch")
		return err
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

	if err := s.save(); err != nil {
		logger.Error().Err(err).Str("scratch_id", id).Msg("Failed to save after removing scratch")
		return err
	}

	logger.Info().Str("scratch_id", id).Int("old_count", oldCount).Int("new_count", newCount).Bool("found", found).Msg("Scratch removal completed")
	return nil
}

func (s *Store) UpdateScratch(scratchToUpdate Scratch) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	logger := logging.GetLogger("store")
	logger.Info().Str("scratch_id", scratchToUpdate.ID).Str("title", scratchToUpdate.Title).Msg("Updating scratch")

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

	if err := s.save(); err != nil {
		logger.Error().Err(err).Str("scratch_id", scratchToUpdate.ID).Msg("Failed to save after updating scratch")
		return err
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

	filePath := cfg.FileSystem.Join(path, id)
	logger.Debug().Str("scratch_id", id).Str("file_path", filePath).Msg("Resolved scratch file path")
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

// withFileLock performs an operation with file-based locking to prevent concurrent metadata corruption
func (s *Store) withFileLock(operation func() error) error {
	logger := logging.GetLogger("store")

	metadataPath, err := s.getMetadataPathWithStore()
	if err != nil {
		return err
	}

	lock := filelock.New(metadataPath)

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
		s.scratches = scratches
		return nil
	})
}
