package symlinks

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/arthur-debert/padz/pkg/store"
)

// Manager handles creating and maintaining symlinks to pad files
type Manager struct {
	store   *store.Store
	linkDir string
}

// NewManager creates a new symlink manager
func NewManager(s *store.Store, linkDir string) (*Manager, error) {
	// Default to ~/.padz-links if not specified
	if linkDir == "" {
		home, err := os.UserHomeDir()
		if err != nil {
			return nil, err
		}
		linkDir = filepath.Join(home, ".padz-links")
	}

	if err := os.MkdirAll(linkDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create link directory: %w", err)
	}

	return &Manager{
		store:   s,
		linkDir: linkDir,
	}, nil
}

// Update refreshes all symlinks based on current pads
func (m *Manager) Update() error {
	// Clean existing links
	if err := m.clean(); err != nil {
		return err
	}

	// Create new links
	scratches := m.store.GetScratches()
	for i, scratch := range scratches {
		// Create numeric link (1-based index)
		numLink := filepath.Join(m.linkDir, fmt.Sprintf("%d", i+1))
		target, err := store.GetScratchFilePath(scratch.ID)
		if err != nil {
			continue
		}

		// Remove existing link if it exists
		_ = os.Remove(numLink)
		if err := os.Symlink(target, numLink); err != nil {
			return fmt.Errorf("failed to create numeric link: %w", err)
		}

		// Create ID link
		idLink := filepath.Join(m.linkDir, scratch.ID)
		_ = os.Remove(idLink) // Remove existing link if it exists
		if err := os.Symlink(target, idLink); err != nil {
			return fmt.Errorf("failed to create ID link: %w", err)
		}

		// Create title link if title exists
		if scratch.Title != "" && scratch.Title != "Untitled" {
			titleLink := filepath.Join(m.linkDir, SanitizeFilename(scratch.Title))
			// Handle collisions by appending part of ID
			if _, err := os.Stat(titleLink); err == nil {
				titleLink = filepath.Join(m.linkDir, fmt.Sprintf("%s-%s", SanitizeFilename(scratch.Title), scratch.ID[:8]))
			}
			if err := os.Symlink(target, titleLink); err != nil {
				// Non-fatal - title links are optional
				continue
			}
		}
	}

	return nil
}

// GetLinkDir returns the directory containing symlinks
func (m *Manager) GetLinkDir() string {
	return m.linkDir
}

// clean removes all existing files in the link directory
func (m *Manager) clean() error {
	entries, err := os.ReadDir(m.linkDir)
	if err != nil && !os.IsNotExist(err) {
		return err
	}

	for _, entry := range entries {
		path := filepath.Join(m.linkDir, entry.Name())
		// Remove all files (symlinks, regular files, etc)
		// This ensures we start fresh
		_ = os.Remove(path)
	}

	return nil
}

// SanitizeFilename makes a title safe for use as a filename
func SanitizeFilename(name string) string {
	// Replace problematic characters
	replacements := []struct {
		old, new string
	}{
		{"/", "-"},
		{"\\", "-"},
		{":", "-"},
		{"*", ""},
		{"?", ""},
		{"\"", ""},
		{"<", ""},
		{">", ""},
		{"|", "-"},
		{"\n", " "},
		{"\r", " "},
		{"\t", " "},
	}

	result := name
	for _, r := range replacements {
		result = strings.ReplaceAll(result, r.old, r.new)
	}

	// Collapse multiple spaces/dashes
	result = strings.TrimSpace(result)
	for strings.Contains(result, "  ") {
		result = strings.ReplaceAll(result, "  ", " ")
	}
	for strings.Contains(result, "--") {
		result = strings.ReplaceAll(result, "--", "-")
	}

	// Limit length
	if len(result) > 50 {
		result = result[:50]
	}

	return result
}
