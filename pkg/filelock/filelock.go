package filelock

import (
	"fmt"
	"os"
	"path/filepath"
	"time"
)

// FileLock provides file-based locking mechanism
type FileLock struct {
	path string
}

// New creates a new file lock for the given path
func New(path string) *FileLock {
	return &FileLock{
		path: path + ".lock",
	}
}

// Lock attempts to acquire the lock with timeout
func (f *FileLock) Lock(timeout time.Duration) error {
	deadline := time.Now().Add(timeout)

	for time.Now().Before(deadline) {
		// Try to create lock file exclusively
		file, err := os.OpenFile(f.path, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0644)
		if err == nil {
			// Write PID to help with debugging
			_, _ = fmt.Fprintf(file, "%d\n", os.Getpid())
			_ = file.Close()
			return nil
		}

		// If file exists, check if it's stale (older than 1 minute)
		if os.IsExist(err) {
			if info, statErr := os.Stat(f.path); statErr == nil {
				if time.Since(info.ModTime()) > time.Minute {
					// Stale lock, try to remove it
					_ = os.Remove(f.path)
					continue
				}
			}
		}

		// Wait a bit before retrying
		time.Sleep(10 * time.Millisecond)
	}

	return fmt.Errorf("failed to acquire lock on %s within %v", filepath.Base(f.path), timeout)
}

// Unlock releases the lock
func (f *FileLock) Unlock() error {
	return os.Remove(f.path)
}
