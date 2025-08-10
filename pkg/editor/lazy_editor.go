package editor

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"github.com/adrg/xdg"
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/logging"
)

// LaunchAndExit launches the editor with a file in the final location and exits immediately
func LaunchAndExit(scratchID string, content []byte) error {
	return LaunchAndExitWithConfig(scratchID, content, config.GetConfig())
}

// LaunchAndExitWithConfig launches the editor with a file in the final location and exits immediately
func LaunchAndExitWithConfig(scratchID string, content []byte, cfg *config.Config) error {
	logger := logging.GetLogger("editor")

	editor := os.Getenv("EDITOR")
	if editor == "" {
		editor = "vim" // default to vim
		logger.Debug().Str("editor", editor).Msg("Using default editor")
	} else {
		logger.Debug().Str("editor", editor).Msg("Using EDITOR environment variable")
	}

	// Get the final path for the scratch file
	var path string
	if cfg.DataPath != "" {
		// Use configured path for testing
		path = filepath.Join(cfg.DataPath, "scratch")
	} else {
		// Use XDG for production
		xdgPath, err := xdg.DataFile("scratch")
		if err != nil {
			logger.Error().Err(err).Msg("Failed to get XDG data file path")
			return err
		}
		path = xdgPath
	}

	var finalPath string
	// Check if new metadata system is in use
	filesPath := filepath.Join(path, "files")
	if _, err := os.Stat(filesPath); err == nil {
		// New structure exists, use it
		finalPath = filepath.Join(filesPath, scratchID)
		logger.Debug().Str("scratch_id", scratchID).Str("file_path", finalPath).Msg("Using new file structure")
	} else {
		// Fall back to old structure
		finalPath = filepath.Join(path, scratchID)
		logger.Debug().Str("scratch_id", scratchID).Str("file_path", finalPath).Msg("Using legacy file structure")
	}

	// Ensure the directory exists
	dir := filepath.Dir(finalPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		logger.Error().Err(err).Str("dir", dir).Msg("Failed to create directory")
		return err
	}

	// Write content to final location
	if err := os.WriteFile(finalPath, content, 0644); err != nil {
		logger.Error().Err(err).Str("path", finalPath).Msg("Failed to write content to file")
		return err
	}

	logger.Info().Str("editor", editor).Str("file", finalPath).Msg("Launching editor (will exit immediately)")

	// Launch the editor in the background
	cmd := exec.Command(editor, finalPath)

	// Detach from the current process
	cmd.Stdin = nil
	cmd.Stdout = nil
	cmd.Stderr = nil

	// Start the editor without waiting
	if err := cmd.Start(); err != nil {
		logger.Error().Err(err).Str("editor", editor).Str("file", finalPath).Msg("Failed to launch editor")
		return err
	}

	// Release the process - it will continue running after padz exits
	if err := cmd.Process.Release(); err != nil {
		logger.Warn().Err(err).Msg("Failed to release editor process")
		// This is not critical, continue
	}

	logger.Info().Str("editor", editor).Str("file", finalPath).Msg("Editor launched successfully, padz exiting")
	fmt.Printf("Opening %s in %s...\n", finalPath, editor)

	return nil
}
