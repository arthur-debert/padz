package store

import (
	"encoding/json"
	"os"
	"sync"

	"github.com/adrg/xdg"
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
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
	return NewStoreWithConfig(config.GetConfig())
}

func NewStoreWithConfig(cfg *config.Config) (*Store, error) {
	store := &Store{
		fs:  cfg.FileSystem,
		cfg: cfg,
	}
	if err := store.load(); err != nil {
		return nil, err
	}
	return store, nil
}

func (s *Store) GetScratches() []Scratch {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.scratches
}

func (s *Store) SaveScratches(scratches []Scratch) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.scratches = scratches
	return s.save()
}

func (s *Store) load() error {
	s.mu.Lock()
	defer s.mu.Unlock()

	path, err := s.getMetadataPathWithStore()
	if err != nil {
		return err
	}

	if _, err := s.fs.Stat(path); os.IsNotExist(err) {
		s.scratches = []Scratch{}
		return nil
	}

	data, err := s.fs.ReadFile(path)
	if err != nil {
		return err
	}

	return json.Unmarshal(data, &s.scratches)
}

func (s *Store) save() error {
	path, err := s.getMetadataPathWithStore()
	if err != nil {
		return err
	}

	data, err := json.MarshalIndent(s.scratches, "", "  ")
	if err != nil {
		return err
	}

	return s.fs.WriteFile(path, data, 0644)
}

func (s *Store) AddScratch(scratch Scratch) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.scratches = append(s.scratches, scratch)
	return s.save()
}

func (s *Store) RemoveScratch(id string) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	var newScratches []Scratch
	for _, scratch := range s.scratches {
		if scratch.ID != id {
			newScratches = append(newScratches, scratch)
		}
	}
	s.scratches = newScratches
	return s.save()
}

func (s *Store) UpdateScratch(scratchToUpdate Scratch) error {
	s.mu.Lock()
	defer s.mu.Unlock()

	for i, scratch := range s.scratches {
		if scratch.ID == scratchToUpdate.ID {
			s.scratches[i] = scratchToUpdate
			break
		}
	}
	return s.save()
}

func GetScratchPath() (string, error) {
	cfg := config.GetConfig()
	return GetScratchPathWithConfig(cfg)
}

func GetScratchPathWithConfig(cfg *config.Config) (string, error) {
	var path string
	var err error

	if cfg.DataPath != "" {
		// Use configured path for testing
		path = cfg.FileSystem.Join(cfg.DataPath, dataDirName)
	} else {
		// Use XDG for production
		path, err = xdg.DataFile(dataDirName)
		if err != nil {
			return "", err
		}
	}

	if err := cfg.FileSystem.MkdirAll(path, 0755); err != nil {
		return "", err
	}
	return path, nil
}

func GetScratchFilePath(id string) (string, error) {
	cfg := config.GetConfig()
	return GetScratchFilePathWithConfig(id, cfg)
}

func GetScratchFilePathWithConfig(id string, cfg *config.Config) (string, error) {
	path, err := GetScratchPathWithConfig(cfg)
	if err != nil {
		return "", err
	}
	return cfg.FileSystem.Join(path, id), nil
}

func getMetadataPathWithConfig(cfg *config.Config) (string, error) {
	path, err := GetScratchPathWithConfig(cfg)
	if err != nil {
		return "", err
	}
	return cfg.FileSystem.Join(path, metadataFileName), nil
}

func (s *Store) getMetadataPathWithStore() (string, error) {
	return getMetadataPathWithConfig(s.cfg)
}
