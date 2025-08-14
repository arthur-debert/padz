package editor

import (
	"os"
	"os/exec"
	"strings"

	"github.com/arthur-debert/padz/pkg/logging"
)

func OpenInEditor(content []byte) ([]byte, error) {
	return OpenInEditorWithExtension(content, "")
}

// OpenInEditorWithExtension opens content in editor with optional extension hint
func OpenInEditorWithExtension(content []byte, extensionHint string) ([]byte, error) {
	logger := logging.GetLogger("editor")

	editor := GetEditor()

	logger.Info().Str("editor", editor).Int("content_size", len(content)).Msg("Starting editor session")

	// Determine file extension
	extension := extensionHint
	if extension == "" {
		// Default to .txt if no hint provided
		extension = ".txt"
		// Check if content starts with # for markdown
		contentStr := strings.TrimSpace(string(content))
		if strings.HasPrefix(contentStr, "#") {
			extension = ".md"
		}
	}

	tmpfile, err := os.CreateTemp("", "scratch-*"+extension)
	if err != nil {
		logger.Error().Err(err).Msg("Failed to create temporary file")
		return nil, err
	}

	tmpPath := tmpfile.Name()
	logger.Debug().Str("temp_file", tmpPath).Msg("Created temporary file")

	defer func() {
		if removeErr := os.Remove(tmpPath); removeErr != nil {
			logger.Warn().Err(removeErr).Str("temp_file", tmpPath).Msg("Failed to cleanup temporary file")
		} else {
			logger.Debug().Str("temp_file", tmpPath).Msg("Temporary file cleaned up successfully")
		}
	}()

	if len(content) > 0 {
		logger.Debug().Int("bytes_to_write", len(content)).Str("temp_file", tmpPath).Msg("Writing initial content to temp file")
		if _, err := tmpfile.Write(content); err != nil {
			logger.Error().Err(err).Str("temp_file", tmpPath).Int("content_size", len(content)).Msg("Failed to write content to temp file")
			return nil, err
		}
		logger.Debug().Int("bytes_written", len(content)).Str("temp_file", tmpPath).Msg("Initial content written successfully")
	} else {
		logger.Debug().Str("temp_file", tmpPath).Msg("No initial content to write")
	}

	if err := tmpfile.Close(); err != nil {
		logger.Error().Err(err).Str("temp_file", tmpPath).Msg("Failed to close temp file")
		return nil, err
	}

	logger.Debug().Str("temp_file", tmpPath).Msg("Temp file closed, ready for editor")

	cmd := exec.Command(editor, tmpPath)
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	logger.Info().Str("editor", editor).Str("temp_file", tmpPath).Msg("Launching editor")

	if err := cmd.Run(); err != nil {
		logger.Error().Err(err).Str("editor", editor).Str("temp_file", tmpPath).Msg("Editor execution failed")
		return nil, err
	}

	logger.Info().Str("editor", editor).Str("temp_file", tmpPath).Msg("Editor execution completed")

	// Read the modified content
	logger.Debug().Str("temp_file", tmpPath).Msg("Reading modified content from temp file")
	modifiedContent, err := os.ReadFile(tmpPath)
	if err != nil {
		logger.Error().Err(err).Str("temp_file", tmpPath).Msg("Failed to read modified content")
		return nil, err
	}

	logger.Info().Str("editor", editor).Int("original_size", len(content)).Int("modified_size", len(modifiedContent)).Msg("Editor session completed successfully")
	logger.Debug().Str("temp_file", tmpPath).Int("bytes_read", len(modifiedContent)).Msg("Modified content read successfully")

	return modifiedContent, nil
}
