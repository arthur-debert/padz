package editor

import (
	"os"

	"github.com/arthur-debert/padz/pkg/logging"
)

// GetEditor returns the editor to use, defaulting to vim if EDITOR is not set
func GetEditor() string {
	logger := logging.GetLogger("editor")

	editor := os.Getenv("EDITOR")
	if editor == "" {
		editor = "vim" // default to vim
		logger.Debug().Str("editor", editor).Msg("Using default editor")
	} else {
		logger.Debug().Str("editor", editor).Msg("Using EDITOR environment variable")
	}

	return editor
}
