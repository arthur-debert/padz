package commands

import (
	"bufio"
	"bytes"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
)

// RecoveryResult contains the results of a recovery operation
type RecoveryResult struct {
	// Orphaned files found (files without metadata entries)
	OrphanedFiles []OrphanedFile `json:"orphaned_files"`

	// Metadata entries without files
	MissingFiles []MissingFile `json:"missing_files"`

	// Successfully recovered files
	RecoveredFiles []RecoveredFile `json:"recovered_files"`

	// Errors encountered during recovery
	Errors []RecoveryError `json:"errors"`

	// Summary statistics
	Summary RecoverySummary `json:"summary"`
}

// OrphanedFile represents a file found on disk without metadata
type OrphanedFile struct {
	ID      string    `json:"id"`
	Path    string    `json:"path"`
	Size    int64     `json:"size"`
	ModTime time.Time `json:"mod_time"`
	Title   string    `json:"title"`   // Extracted from content
	Preview string    `json:"preview"` // First few lines
}

// MissingFile represents a metadata entry without a corresponding file
type MissingFile struct {
	ID        string    `json:"id"`
	Project   string    `json:"project"`
	Title     string    `json:"title"`
	CreatedAt time.Time `json:"created_at"`
}

// RecoveredFile represents a successfully recovered file
type RecoveredFile struct {
	ID        string    `json:"id"`
	Project   string    `json:"project"`
	Title     string    `json:"title"`
	CreatedAt time.Time `json:"created_at"`
	Source    string    `json:"source"` // "orphaned" or "reconstructed"
}

// RecoveryError represents an error during recovery
type RecoveryError struct {
	Type    string `json:"type"`
	Message string `json:"message"`
	FileID  string `json:"file_id,omitempty"`
}

// RecoverySummary contains summary statistics
type RecoverySummary struct {
	TotalOrphaned  int       `json:"total_orphaned"`
	TotalMissing   int       `json:"total_missing"`
	TotalRecovered int       `json:"total_recovered"`
	TotalErrors    int       `json:"total_errors"`
	StartTime      time.Time `json:"start_time"`
	EndTime        time.Time `json:"end_time"`
	Duration       string    `json:"duration"`
}

// RecoveryOptions configures the recovery behavior
type RecoveryOptions struct {
	DryRun         bool   // Don't make changes, just report
	RecoverOrphans bool   // Recover orphaned files
	CleanMissing   bool   // Remove metadata entries without files
	DefaultProject string // Project to use for orphaned files (default: "recovered")
}

// Recover performs a recovery operation on the scratch store
func Recover(s *store.Store, options RecoveryOptions) (*RecoveryResult, error) {
	log.Info().
		Bool("dry_run", options.DryRun).
		Bool("recover_orphans", options.RecoverOrphans).
		Bool("clean_missing", options.CleanMissing).
		Str("default_project", options.DefaultProject).
		Msg("Starting recovery operation")

	startTime := time.Now()
	result := &RecoveryResult{
		OrphanedFiles:  []OrphanedFile{},
		MissingFiles:   []MissingFile{},
		RecoveredFiles: []RecoveredFile{},
		Errors:         []RecoveryError{},
		Summary: RecoverySummary{
			StartTime: startTime,
		},
	}

	// Set default project if not specified
	if options.DefaultProject == "" {
		options.DefaultProject = "recovered"
	}

	// Get scratch directory path
	scratchPath, err := store.GetScratchPath()
	if err != nil {
		log.Error().Err(err).Msg("Failed to get scratch path")
		return nil, fmt.Errorf("failed to get scratch path: %w", err)
	}

	// Find orphaned files
	orphaned, err := findOrphanedFiles(s, scratchPath)
	if err != nil {
		log.Error().Err(err).Msg("Failed to find orphaned files")
		result.Errors = append(result.Errors, RecoveryError{
			Type:    "scan_error",
			Message: fmt.Sprintf("Failed to scan for orphaned files: %v", err),
		})
	} else {
		result.OrphanedFiles = orphaned
		log.Info().Int("count", len(orphaned)).Msg("Found orphaned files")
	}

	// Find missing files
	missing := findMissingFiles(s, scratchPath)
	result.MissingFiles = missing
	log.Info().Int("count", len(missing)).Msg("Found missing files")

	// Recover orphaned files if requested
	if options.RecoverOrphans && !options.DryRun {
		for _, orphan := range result.OrphanedFiles {
			if err := recoverOrphanedFile(s, orphan, options.DefaultProject); err != nil {
				log.Error().
					Err(err).
					Str("file_id", orphan.ID).
					Msg("Failed to recover orphaned file")
				result.Errors = append(result.Errors, RecoveryError{
					Type:    "recovery_error",
					Message: fmt.Sprintf("Failed to recover file %s: %v", orphan.ID, err),
					FileID:  orphan.ID,
				})
			} else {
				log.Info().
					Str("file_id", orphan.ID).
					Str("title", orphan.Title).
					Msg("Recovered orphaned file")
				result.RecoveredFiles = append(result.RecoveredFiles, RecoveredFile{
					ID:        orphan.ID,
					Project:   options.DefaultProject,
					Title:     orphan.Title,
					CreatedAt: orphan.ModTime,
					Source:    "orphaned",
				})
			}
		}
	}

	// Clean missing metadata entries if requested
	if options.CleanMissing && !options.DryRun {
		scratches := s.GetScratches()
		cleanedScratches := []store.Scratch{}

		// Create a map of missing IDs for quick lookup
		missingIDs := make(map[string]bool)
		for _, m := range missing {
			missingIDs[m.ID] = true
		}

		// Filter out missing entries
		for _, scratch := range scratches {
			if !missingIDs[scratch.ID] {
				cleanedScratches = append(cleanedScratches, scratch)
			} else {
				log.Info().
					Str("id", scratch.ID).
					Str("title", scratch.Title).
					Msg("Removing metadata entry without file")
			}
		}

		if err := s.SaveScratchesAtomic(cleanedScratches); err != nil {
			log.Error().Err(err).Msg("Failed to save cleaned metadata")
			result.Errors = append(result.Errors, RecoveryError{
				Type:    "save_error",
				Message: fmt.Sprintf("Failed to save cleaned metadata: %v", err),
			})
		}
	}

	// Update summary
	endTime := time.Now()
	result.Summary = RecoverySummary{
		TotalOrphaned:  len(result.OrphanedFiles),
		TotalMissing:   len(result.MissingFiles),
		TotalRecovered: len(result.RecoveredFiles),
		TotalErrors:    len(result.Errors),
		StartTime:      startTime,
		EndTime:        endTime,
		Duration:       endTime.Sub(startTime).String(),
	}

	log.Info().
		Int("orphaned", result.Summary.TotalOrphaned).
		Int("missing", result.Summary.TotalMissing).
		Int("recovered", result.Summary.TotalRecovered).
		Int("errors", result.Summary.TotalErrors).
		Str("duration", result.Summary.Duration).
		Msg("Recovery operation completed")

	return result, nil
}

// findOrphanedFiles finds files on disk without metadata entries
func findOrphanedFiles(s *store.Store, scratchPath string) ([]OrphanedFile, error) {
	// Get all metadata IDs
	scratches := s.GetScratches()
	metadataIDs := make(map[string]bool)
	for _, scratch := range scratches {
		metadataIDs[scratch.ID] = true
	}

	// Scan directory for files
	orphaned := []OrphanedFile{}
	entries, err := os.ReadDir(scratchPath)
	if err != nil {
		return nil, fmt.Errorf("failed to read scratch directory: %w", err)
	}

	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}

		// Skip metadata.json
		if entry.Name() == "metadata.json" {
			continue
		}

		// Check if file has metadata entry
		fileID := entry.Name()
		if !metadataIDs[fileID] {
			// Found orphaned file
			filePath := filepath.Join(scratchPath, fileID)
			info, err := entry.Info()
			if err != nil {
				log.Warn().
					Err(err).
					Str("file", fileID).
					Msg("Failed to get file info")
				continue
			}

			// Read file to extract title and preview
			title, preview, err := extractTitleAndPreview(filePath)
			if err != nil {
				log.Warn().
					Err(err).
					Str("file", fileID).
					Msg("Failed to extract title and preview")
				title = "Unknown"
				preview = ""
			}

			orphaned = append(orphaned, OrphanedFile{
				ID:      fileID,
				Path:    filePath,
				Size:    info.Size(),
				ModTime: info.ModTime(),
				Title:   title,
				Preview: preview,
			})
		}
	}

	return orphaned, nil
}

// findMissingFiles finds metadata entries without corresponding files
func findMissingFiles(s *store.Store, scratchPath string) []MissingFile {
	missing := []MissingFile{}
	scratches := s.GetScratches()

	for _, scratch := range scratches {
		filePath := filepath.Join(scratchPath, scratch.ID)
		if _, err := os.Stat(filePath); os.IsNotExist(err) {
			missing = append(missing, MissingFile{
				ID:        scratch.ID,
				Project:   scratch.Project,
				Title:     scratch.Title,
				CreatedAt: scratch.CreatedAt,
			})
		}
	}

	return missing
}

// extractTitleAndPreview reads a file and extracts the title and preview
func extractTitleAndPreview(filePath string) (string, string, error) {
	file, err := os.Open(filePath)
	if err != nil {
		return "", "", err
	}
	defer func() {
		if err := file.Close(); err != nil {
			log.Warn().Err(err).Msg("Failed to close file")
		}
	}()

	reader := bufio.NewReader(file)
	var title string
	var previewLines []string
	lineCount := 0
	maxPreviewLines := 3

	for lineCount < maxPreviewLines {
		line, err := reader.ReadString('\n')
		if err != nil {
			if err == io.EOF && line != "" {
				// Handle last line without newline
				if lineCount == 0 && title == "" {
					title = line
				}
				previewLines = append(previewLines, line)
			}
			break
		}

		// First non-empty line is the title
		trimmed := bytes.TrimSpace([]byte(line))
		if lineCount == 0 && len(trimmed) > 0 && title == "" {
			title = string(trimmed)
		}

		previewLines = append(previewLines, line)
		lineCount++
	}

	if title == "" {
		title = "Untitled"
	}

	preview := ""
	for i, line := range previewLines {
		preview += line
		if i < len(previewLines)-1 && !bytes.HasSuffix([]byte(line), []byte("\n")) {
			preview += "\n"
		}
	}

	return title, preview, nil
}

// recoverOrphanedFile adds an orphaned file back to metadata
func recoverOrphanedFile(s *store.Store, orphan OrphanedFile, project string) error {
	scratch := store.Scratch{
		ID:        orphan.ID,
		Project:   project,
		Title:     orphan.Title,
		CreatedAt: orphan.ModTime, // Use file modification time as creation time
	}

	return s.AddScratch(scratch)
}

// RecoverWithStoreManager performs a recovery operation using StoreManager
func RecoverWithStoreManager(workingDir string, globalFlag bool, options RecoveryOptions) (*RecoveryResult, error) {
	sm := store.NewStoreManager()

	log.Info().
		Bool("dry_run", options.DryRun).
		Bool("recover_orphans", options.RecoverOrphans).
		Bool("clean_missing", options.CleanMissing).
		Str("default_project", options.DefaultProject).
		Bool("global", globalFlag).
		Msg("Starting recovery operation with StoreManager")

	startTime := time.Now()
	result := &RecoveryResult{
		OrphanedFiles:  []OrphanedFile{},
		MissingFiles:   []MissingFile{},
		RecoveredFiles: []RecoveredFile{},
		Errors:         []RecoveryError{},
		Summary: RecoverySummary{
			StartTime: startTime,
		},
	}

	// Set default project if not specified
	if options.DefaultProject == "" {
		options.DefaultProject = "recovered"
	}

	// Determine which store to recover
	var targetStore *store.Store
	var storeScope string
	var err error

	if globalFlag {
		targetStore, err = sm.GetGlobalStore()
		if err != nil {
			return nil, fmt.Errorf("failed to get global store: %w", err)
		}
		storeScope = "global"
	} else {
		// Use current project store
		targetStore, storeScope, err = sm.GetCurrentStore(workingDir, false)
		if err != nil {
			return nil, fmt.Errorf("failed to get current store: %w", err)
		}
	}

	log.Info().Str("scope", storeScope).Msg("Running recovery on store")

	// Run the existing recovery logic on the target store
	storeResult, err := Recover(targetStore, options)
	if err != nil {
		return nil, fmt.Errorf("recovery failed for %s store: %w", storeScope, err)
	}

	// Copy the results (since we're only operating on one store at a time)
	*result = *storeResult

	log.Info().
		Int("orphaned", result.Summary.TotalOrphaned).
		Int("missing", result.Summary.TotalMissing).
		Int("recovered", result.Summary.TotalRecovered).
		Int("errors", result.Summary.TotalErrors).
		Str("duration", result.Summary.Duration).
		Str("scope", storeScope).
		Msg("Recovery operation completed for store")

	return result, nil
}
