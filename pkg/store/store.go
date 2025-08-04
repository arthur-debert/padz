package store

import (
	"encoding/json"
	"os"
	"path/filepath"
	"sync"

	"github.com/adrg/xdg"
)

const (
	dataDirName      = "scratch"
	metadataFileName = "metadata.json"
)

type Store struct {
	mu        sync.Mutex
	scratches []Scratch
}

func NewStore() (*Store, error) {
	store := &Store{}
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

	path, err := getMetadataPath()
	if err != nil {
		return err
	}

	if _, err := os.Stat(path); os.IsNotExist(err) {
		s.scratches = []Scratch{}
		return nil
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}

	return json.Unmarshal(data, &s.scratches)
}

func (s *Store) save() error {
	path, err := getMetadataPath()
	if err != nil {
		return err
	}

	data, err := json.MarshalIndent(s.scratches, "", "  ")
	if err != nil {
		return err
	}

	return os.WriteFile(path, data, 0644)
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
	path, err := xdg.DataFile(dataDirName)
	if err != nil {
		return "", err
	}
	if err := os.MkdirAll(path, 0755); err != nil {
		return "", err
	}
	return path, nil
}

func GetScratchFilePath(id string) (string, error) {
	path, err := GetScratchPath()
	if err != nil {
		return "", err
	}
	return filepath.Join(path, id), nil
}

func getMetadataPath() (string, error) {
	path, err := GetScratchPath()
	if err != nil {
		return "", err
	}
	return filepath.Join(path, metadataFileName), nil
}
