package config

import (
	"github.com/adrg/xdg"
	"github.com/arthur-debert/padz/pkg/filesystem"
)

// NakedIntCommand defines which command to run when using naked int invocation (e.g., padz 2)
// Valid values: "view" or "open"
const NakedIntCommand = "view"

// Config holds the application configuration
type Config struct {
	FileSystem filesystem.FileSystem
	DataPath   string // Base path for data storage
}

// DefaultConfig returns the default configuration for production use
func DefaultConfig() *Config {
	// Resolve XDG data path at config initialization time
	dataPath, err := xdg.DataFile("scratch")
	if err != nil {
		// Fallback to a reasonable default if XDG fails
		dataPath = "~/.local/share/scratch"
	}

	return &Config{
		FileSystem: filesystem.NewOSFileSystem(),
		DataPath:   dataPath,
	}
}

// TestConfig returns a configuration suitable for testing
func TestConfig() *Config {
	return &Config{
		FileSystem: filesystem.NewMemoryFileSystem(),
		DataPath:   "/test/data", // Use a fixed test path
	}
}

var globalConfig *Config

// GetConfig returns the global configuration
func GetConfig() *Config {
	if globalConfig == nil {
		globalConfig = DefaultConfig()
	}
	return globalConfig
}

// SetConfig sets the global configuration
func SetConfig(cfg *Config) {
	globalConfig = cfg
}

// ResetConfig resets the global configuration to default
func ResetConfig() {
	globalConfig = nil
}
