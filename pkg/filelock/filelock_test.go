package filelock

import (
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"
)

func TestFileLock(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "filelock_test")
	if err != nil {
		t.Fatal(err)
	}
	defer func() { _ = os.RemoveAll(tmpDir) }()

	lockPath := filepath.Join(tmpDir, "test.lock")

	t.Run("basic lock and unlock", func(t *testing.T) {
		lock := New(lockPath)

		// Acquire lock
		if err := lock.Lock(1 * time.Second); err != nil {
			t.Fatalf("Failed to acquire lock: %v", err)
		}

		// Check lock file exists
		if _, err := os.Stat(lockPath + ".lock"); os.IsNotExist(err) {
			t.Fatal("Lock file not created")
		}

		// Release lock
		if err := lock.Unlock(); err != nil {
			t.Fatalf("Failed to release lock: %v", err)
		}

		// Check lock file removed
		if _, err := os.Stat(lockPath + ".lock"); !os.IsNotExist(err) {
			t.Fatal("Lock file not removed")
		}
	})

	t.Run("concurrent access prevention", func(t *testing.T) {
		lock1 := New(lockPath)
		lock2 := New(lockPath)

		// First lock should succeed
		if err := lock1.Lock(1 * time.Second); err != nil {
			t.Fatalf("First lock failed: %v", err)
		}
		defer func() { _ = lock1.Unlock() }()

		// Second lock should timeout
		if err := lock2.Lock(100 * time.Millisecond); err == nil {
			t.Fatal("Second lock should have failed")
		}
	})

	t.Run("stale lock removal", func(t *testing.T) {
		// Create a stale lock file
		staleLockPath := lockPath + ".lock"
		if err := os.WriteFile(staleLockPath, []byte("999999"), 0644); err != nil {
			t.Fatal(err)
		}

		// Modify time to make it stale
		oldTime := time.Now().Add(-2 * time.Minute)
		if err := os.Chtimes(staleLockPath, oldTime, oldTime); err != nil {
			t.Fatal(err)
		}

		// Should be able to acquire lock
		lock := New(lockPath)
		if err := lock.Lock(1 * time.Second); err != nil {
			t.Fatalf("Failed to acquire lock with stale file: %v", err)
		}
		defer func() { _ = lock.Unlock() }()
	})

	t.Run("concurrent operations", func(t *testing.T) {
		const numGoroutines = 10
		counter := 0
		var wg sync.WaitGroup
		var mu sync.Mutex

		for i := 0; i < numGoroutines; i++ {
			wg.Add(1)
			go func() {
				defer wg.Done()

				lock := New(lockPath)
				if err := lock.Lock(5 * time.Second); err != nil {
					t.Errorf("Failed to acquire lock: %v", err)
					return
				}
				defer func() { _ = lock.Unlock() }()

				// Simulate critical section
				mu.Lock()
				counter++
				mu.Unlock()
				time.Sleep(10 * time.Millisecond)
			}()
		}

		wg.Wait()

		if counter != numGoroutines {
			t.Fatalf("Expected counter to be %d, got %d", numGoroutines, counter)
		}
	})
}
