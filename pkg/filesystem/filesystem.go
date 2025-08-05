package filesystem

import (
	"os"
	"path/filepath"
	"sync"
	"time"
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
	return os.WriteFile(path, data, perm)
}

func (fs *OSFileSystem) ReadFile(path string) ([]byte, error) {
	return os.ReadFile(path)
}

func (fs *OSFileSystem) Stat(path string) (os.FileInfo, error) {
	return os.Stat(path)
}

func (fs *OSFileSystem) Remove(path string) error {
	return os.Remove(path)
}

func (fs *OSFileSystem) MkdirAll(path string, perm os.FileMode) error {
	return os.MkdirAll(path, perm)
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
	fs.mu.Lock()
	defer fs.mu.Unlock()

	// Ensure parent directory exists
	dir := filepath.Dir(path)
	if !fs.dirExists(dir) {
		return &os.PathError{Op: "write", Path: path, Err: os.ErrNotExist}
	}

	// Copy data to avoid mutations
	dataCopy := make([]byte, len(data))
	copy(dataCopy, data)
	fs.files[path] = dataCopy
	return nil
}

func (fs *MemoryFileSystem) ReadFile(path string) ([]byte, error) {
	fs.mu.RLock()
	defer fs.mu.RUnlock()

	data, exists := fs.files[path]
	if !exists {
		return nil, &os.PathError{Op: "read", Path: path, Err: os.ErrNotExist}
	}

	// Return a copy to avoid mutations
	dataCopy := make([]byte, len(data))
	copy(dataCopy, data)
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
	fs.mu.Lock()
	defer fs.mu.Unlock()

	if _, exists := fs.files[path]; exists {
		delete(fs.files, path)
		return nil
	}

	return &os.PathError{Op: "remove", Path: path, Err: os.ErrNotExist}
}

func (fs *MemoryFileSystem) MkdirAll(path string, perm os.FileMode) error {
	fs.mu.Lock()
	defer fs.mu.Unlock()

	// Mark all parent directories as existing
	for p := path; p != "/" && p != "."; p = filepath.Dir(p) {
		fs.dirs[p] = true
	}
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
