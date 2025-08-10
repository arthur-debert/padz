package store

import (
	"crypto/sha1"
	"fmt"
	"os"
	"strings"
	"time"

	"github.com/arthur-debert/padz/pkg/logging"
)

// DiscoveryManager handles auto-discovery of orphaned scratch files
type DiscoveryManager struct {
	store           *Store
	metadataManager *MetadataManager
}

// NewDiscoveryManager creates a new discovery manager
func NewDiscoveryManager(store *Store) *DiscoveryManager {
	return &DiscoveryManager{
		store:           store,
		metadataManager: store.metadataManager,
	}
}

// DiscoverOrphanedFiles finds and indexes files without metadata
func (dm *DiscoveryManager) DiscoverOrphanedFiles() error {
	logger := logging.GetLogger("discovery")
	logger.Info().Msg("Starting auto-discovery of orphaned files")

	// Get the files directory path
	filesPath := dm.metadataManager.GetFilesPath()

	// Check if files directory exists
	if _, err := dm.store.fs.Stat(filesPath); os.IsNotExist(err) {
		logger.Debug().Str("path", filesPath).Msg("Files directory does not exist, nothing to discover")
		return nil
	}

	// List all files in the files directory
	entries, err := dm.store.fs.ReadDir(filesPath)
	if err != nil {
		logger.Error().Err(err).Str("path", filesPath).Msg("Failed to read files directory")
		return err
	}

	// Load current index to check which files already have metadata
	index, err := dm.metadataManager.LoadIndex()
	if err != nil {
		logger.Error().Err(err).Msg("Failed to load index")
		return err
	}

	orphanedCount := 0
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}

		fileID := entry.Name()

		// Skip if metadata already exists
		if _, exists := index.Scratches[fileID]; exists {
			logger.Debug().Str("file_id", fileID).Msg("File already has metadata, skipping")
			continue
		}

		// Check if metadata file exists (double-check)
		if metadata, err := dm.metadataManager.LoadScratchMetadata(fileID); err == nil && metadata != nil {
			logger.Debug().Str("file_id", fileID).Msg("Metadata file exists, updating index")
			// Update index with existing metadata
			index.Scratches[fileID] = IndexEntry{
				Project:   metadata.Project,
				Title:     metadata.Title,
				CreatedAt: metadata.CreatedAt,
			}
			orphanedCount++ // Count this as we need to update the index
			continue
		}

		// This is an orphaned file - generate metadata for it
		logger.Info().Str("file_id", fileID).Msg("Found orphaned file, generating metadata")

		if err := dm.generateMetadataForOrphan(fileID, entry); err != nil {
			logger.Error().Err(err).Str("file_id", fileID).Msg("Failed to generate metadata for orphaned file")
			continue
		}

		// Add to index
		metadata, _ := dm.metadataManager.LoadScratchMetadata(fileID)
		if metadata != nil {
			index.Scratches[fileID] = IndexEntry{
				Project:   metadata.Project,
				Title:     metadata.Title,
				CreatedAt: metadata.CreatedAt,
			}
		}

		orphanedCount++
	}

	// Save updated index if we found any orphans or made any changes
	if orphanedCount > 0 {
		if err := dm.metadataManager.SaveIndex(index); err != nil {
			logger.Error().Err(err).Msg("Failed to save updated index")
			return err
		}
		logger.Info().Int("orphaned_count", orphanedCount).Msg("Discovery completed, generated metadata for orphaned files")
	} else {
		logger.Info().Msg("Discovery completed, no orphaned files found")
	}

	return nil
}

// generateMetadataForOrphan creates metadata for an orphaned file
func (dm *DiscoveryManager) generateMetadataForOrphan(fileID string, entry os.DirEntry) error {
	logger := logging.GetLogger("discovery")

	// Read file content
	filePath := dm.store.fs.Join(dm.metadataManager.GetFilesPath(), fileID)
	content, err := dm.store.fs.ReadFile(filePath)
	if err != nil {
		logger.Error().Err(err).Str("path", filePath).Msg("Failed to read orphaned file")
		return err
	}

	// Verify the file ID matches content hash
	expectedID := fmt.Sprintf("%x", sha1.Sum(content))
	if fileID != expectedID {
		logger.Warn().Str("file_id", fileID).Str("expected_id", expectedID).Msg("File ID does not match content hash")
		// Continue anyway - the file is there and we should track it
	}

	// Get file info for timestamps
	info, err := entry.Info()
	if err != nil {
		logger.Warn().Err(err).Str("file_id", fileID).Msg("Failed to get file info, using current time")
		info = nil
	}

	// Extract title from content
	title := dm.extractTitle(content)

	// Determine timestamps
	var createdAt, updatedAt time.Time
	if info != nil {
		updatedAt = info.ModTime()
		// Use modification time as creation time (best we can do)
		createdAt = info.ModTime()
	} else {
		createdAt = time.Now()
		updatedAt = time.Now()
	}

	// Create metadata
	scratch := &Scratch{
		ID:        fileID,
		Project:   "recovered", // Mark as recovered
		Title:     title,
		CreatedAt: createdAt,
		UpdatedAt: updatedAt,
		Size:      int64(len(content)),
		Checksum:  fmt.Sprintf("sha256:%x", sha1.Sum(content)),
	}

	// Save metadata
	if err := dm.metadataManager.SaveScratchMetadata(scratch); err != nil {
		return err
	}

	// Don't update index here - it will be done in batch by the caller

	// Add to in-memory store
	dm.store.scratches = append(dm.store.scratches, *scratch)

	logger.Info().Str("file_id", fileID).Str("title", title).Msg("Generated metadata for orphaned file")
	return nil
}

// extractTitle extracts a title from the file content
func (dm *DiscoveryManager) extractTitle(content []byte) string {
	// Get first non-empty line
	lines := strings.Split(string(content), "\n")
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if trimmed != "" {
			// Limit title length
			if len(trimmed) > 100 {
				return trimmed[:97] + "..."
			}
			return trimmed
		}
	}

	// If no non-empty lines, use first N characters
	if len(content) > 0 {
		preview := strings.TrimSpace(string(content))
		if preview != "" {
			if len(preview) > 100 {
				return preview[:97] + "..."
			}
			return preview
		}
	}

	return "Untitled (recovered)"
}

// RunDiscoveryBeforeCommand runs discovery before executing a command that needs metadata
func (s *Store) RunDiscoveryBeforeCommand() error {
	// Only run discovery if using new metadata system
	if !s.useNewMetadata {
		return nil
	}

	logger := logging.GetLogger("store")
	logger.Debug().Msg("Running discovery check before command")

	dm := NewDiscoveryManager(s)
	return dm.DiscoverOrphanedFiles()
}
