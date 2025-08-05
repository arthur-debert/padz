package config

import (
	"github.com/arthur-debert/padz/pkg/filesystem"
)

// Config holds the application configuration
type Config struct {
	FileSystem filesystem.FileSystem
	DataPath   string // Base path for data storage
}

// DefaultConfig returns the default configuration for production use
func DefaultConfig() *Config {
	return &Config{
		FileSystem: filesystem.NewOSFileSystem(),
		DataPath:   "", // Empty means use XDG default
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
