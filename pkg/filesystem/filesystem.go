package filesystem

import (
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/arthur-debert/padz/pkg/logging"
)

// FileSystem defines the interface for file system operations
type FileSystem interface {
	// WriteFile writes data to a file
	WriteFile(path string, data []byte, perm os.FileMode) error
	// ReadFile reads the entire file
	ReadFile(path string) ([]byte, error)
	// Stat returns file info
	Stat(path string) (os.FileInfo, error)
	// Remove removes a file
	Remove(path string) error
	// MkdirAll creates a directory path
	MkdirAll(path string, perm os.FileMode) error
	// Join joins path elements
	Join(elem ...string) string
}

// OSFileSystem implements FileSystem using real OS operations
type OSFileSystem struct{}

func NewOSFileSystem() *OSFileSystem {
	return &OSFileSystem{}
}

func (fs *OSFileSystem) WriteFile(path string, data []byte, perm os.FileMode) error {
	logger := logging.GetLogger("filesystem.os")
	logger.Info().Str("path", path).Int("bytes", len(data)).Str("perm", perm.String()).Msg("Writing file")

	if err := os.WriteFile(path, data, perm); err != nil {
		logger.Error().Err(err).Str("path", path).Int("bytes", len(data)).Msg("Failed to write file")
		return err
	}

	logger.Debug().Str("path", path).Int("bytes", len(data)).Msg("File written successfully")
	return nil
}

func (fs *OSFileSystem) ReadFile(path string) ([]byte, error) {
	logger := logging.GetLogger("filesystem.os")
	logger.Debug().Str("path", path).Msg("Reading file")

	data, err := os.ReadFile(path)
	if err != nil {
		logger.Error().Err(err).Str("path", path).Msg("Failed to read file")
		return nil, err
	}

	logger.Debug().Str("path", path).Int("bytes_read", len(data)).Msg("File read successfully")
	return data, nil
}

func (fs *OSFileSystem) Stat(path string) (os.FileInfo, error) {
	logger := logging.GetLogger("filesystem.os")
	logger.Debug().Str("path", path).Msg("Getting file info")

	info, err := os.Stat(path)
	if err != nil {
		logger.Debug().Err(err).Str("path", path).Msg("File stat failed")
		return nil, err
	}

	logger.Debug().Str("path", path).Int64("size", info.Size()).Bool("is_dir", info.IsDir()).Msg("File stat successful")
	return info, nil
}

func (fs *OSFileSystem) Remove(path string) error {
	logger := logging.GetLogger("filesystem.os")
	logger.Info().Str("path", path).Msg("Removing file")

	if err := os.Remove(path); err != nil {
		logger.Error().Err(err).Str("path", path).Msg("Failed to remove file")
		return err
	}

	logger.Info().Str("path", path).Msg("File removed successfully")
	return nil
}

func (fs *OSFileSystem) MkdirAll(path string, perm os.FileMode) error {
	logger := logging.GetLogger("filesystem.os")
	logger.Debug().Str("path", path).Str("perm", perm.String()).Msg("Creating directory path")

	if err := os.MkdirAll(path, perm); err != nil {
		logger.Error().Err(err).Str("path", path).Msg("Failed to create directory path")
		return err
	}

	logger.Debug().Str("path", path).Msg("Directory path created successfully")
	return nil
}

func (fs *OSFileSystem) Join(elem ...string) string {
	return filepath.Join(elem...)
}

// MemoryFileSystem implements FileSystem in memory
type MemoryFileSystem struct {
	mu    sync.RWMutex
	files map[string][]byte
	dirs  map[string]bool
}

func NewMemoryFileSystem() *MemoryFileSystem {
	return &MemoryFileSystem{
		files: make(map[string][]byte),
		dirs:  make(map[string]bool),
	}
}

func (fs *MemoryFileSystem) WriteFile(path string, data []byte, perm os.FileMode) error {
	logger := logging.GetLogger("filesystem.mem")
	logger.Info().Str("path", path).Int("bytes", len(data)).Str("perm", perm.String()).Msg("Writing file to memory")

	fs.mu.Lock()
	defer fs.mu.Unlock()

	// Ensure parent directory exists
	dir := filepath.Dir(path)
	if !fs.dirExists(dir) {
		logger.Error().Str("path", path).Str("parent_dir", dir).Msg("Parent directory does not exist")
		return &os.PathError{Op: "write", Path: path, Err: os.ErrNotExist}
	}

	// Copy data to avoid mutations
	dataCopy := make([]byte, len(data))
	copy(dataCopy, data)
	fs.files[path] = dataCopy

	logger.Debug().Str("path", path).Int("bytes", len(data)).Int("total_files", len(fs.files)).Msg("File written to memory successfully")
	return nil
}

func (fs *MemoryFileSystem) ReadFile(path string) ([]byte, error) {
	logger := logging.GetLogger("filesystem.mem")
	logger.Debug().Str("path", path).Msg("Reading file from memory")

	fs.mu.RLock()
	defer fs.mu.RUnlock()

	data, exists := fs.files[path]
	if !exists {
		logger.Debug().Str("path", path).Msg("File not found in memory")
		return nil, &os.PathError{Op: "read", Path: path, Err: os.ErrNotExist}
	}

	// Return a copy to avoid mutations
	dataCopy := make([]byte, len(data))
	copy(dataCopy, data)

	logger.Debug().Str("path", path).Int("bytes_read", len(data)).Msg("File read from memory successfully")
	return dataCopy, nil
}

func (fs *MemoryFileSystem) Stat(path string) (os.FileInfo, error) {
	fs.mu.RLock()
	defer fs.mu.RUnlock()

	// Check if it's a file
	if data, exists := fs.files[path]; exists {
		return &memFileInfo{
			name: filepath.Base(path),
			size: int64(len(data)),
			mode: 0644,
		}, nil
	}

	// Check if it's a directory
	if fs.dirExists(path) {
		return &memFileInfo{
			name:  filepath.Base(path),
			size:  0,
			mode:  os.ModeDir | 0755,
			isDir: true,
		}, nil
	}

	return nil, &os.PathError{Op: "stat", Path: path, Err: os.ErrNotExist}
}

func (fs *MemoryFileSystem) Remove(path string) error {
	logger := logging.GetLogger("filesystem.mem")
	logger.Info().Str("path", path).Msg("Removing file from memory")

	fs.mu.Lock()
	defer fs.mu.Unlock()

	if _, exists := fs.files[path]; exists {
		delete(fs.files, path)
		logger.Info().Str("path", path).Int("remaining_files", len(fs.files)).Msg("File removed from memory successfully")
		return nil
	}

	logger.Debug().Str("path", path).Msg("File not found for removal")
	return &os.PathError{Op: "remove", Path: path, Err: os.ErrNotExist}
}

func (fs *MemoryFileSystem) MkdirAll(path string, perm os.FileMode) error {
	logger := logging.GetLogger("filesystem.mem")
	logger.Debug().Str("path", path).Str("perm", perm.String()).Msg("Creating directory path in memory")

	fs.mu.Lock()
	defer fs.mu.Unlock()

	// Mark all parent directories as existing
	dirCount := 0
	for p := path; p != "/" && p != "."; p = filepath.Dir(p) {
		if !fs.dirs[p] {
			dirCount++
		}
		fs.dirs[p] = true
	}

	logger.Debug().Str("path", path).Int("dirs_created", dirCount).Int("total_dirs", len(fs.dirs)).Msg("Directory path created in memory successfully")
	return nil
}

func (fs *MemoryFileSystem) Join(elem ...string) string {
	return filepath.Join(elem...)
}

func (fs *MemoryFileSystem) dirExists(path string) bool {
	if path == "/" || path == "." {
		return true
	}
	return fs.dirs[path]
}

// memFileInfo implements os.FileInfo for in-memory files
type memFileInfo struct {
	name  string
	size  int64
	mode  os.FileMode
	isDir bool
}

func (fi *memFileInfo) Name() string       { return fi.name }
func (fi *memFileInfo) Size() int64        { return fi.size }
func (fi *memFileInfo) Mode() os.FileMode  { return fi.mode }
func (fi *memFileInfo) ModTime() time.Time { return time.Now() }
func (fi *memFileInfo) IsDir() bool        { return fi.isDir }
func (fi *memFileInfo) Sys() interface{}   { return nil }

// Reset clears all files and directories in the memory file system
func (fs *MemoryFileSystem) Reset() {
	fs.mu.Lock()
	defer fs.mu.Unlock()
	fs.files = make(map[string][]byte)
	fs.dirs = make(map[string]bool)
}

// GetAllFiles returns all files in the memory file system (for debugging)
func (fs *MemoryFileSystem) GetAllFiles() map[string][]byte {
	fs.mu.RLock()
	defer fs.mu.RUnlock()

	result := make(map[string][]byte)
	for k, v := range fs.files {
		result[k] = v
	}
	return result
}
