package filesystem

import (
	"path/filepath"
	"testing"
)

func TestMemoryFileSystem(t *testing.T) {
	fs := NewMemoryFileSystem()

	t.Run("WriteAndReadFile", func(t *testing.T) {
		testPath := "/test/file.txt"
		testData := []byte("Hello, World!")

		// Create parent directory
		err := fs.MkdirAll(filepath.Dir(testPath), 0755)
		if err != nil {
			t.Fatalf("Failed to create directory: %v", err)
		}

		// Write file
		err = fs.WriteFile(testPath, testData, 0644)
		if err != nil {
			t.Fatalf("Failed to write file: %v", err)
		}

		// Read file
		data, err := fs.ReadFile(testPath)
		if err != nil {
			t.Fatalf("Failed to read file: %v", err)
		}

		if string(data) != string(testData) {
			t.Errorf("Expected %s, got %s", testData, data)
		}
	})

	t.Run("ReadNonExistentFile", func(t *testing.T) {
		_, err := fs.ReadFile("/nonexistent/file.txt")
		if err == nil {
			t.Error("Expected error reading non-existent file")
		}
	})

	t.Run("WriteWithoutDirectory", func(t *testing.T) {
		err := fs.WriteFile("/nodir/file.txt", []byte("test"), 0644)
		if err == nil {
			t.Error("Expected error writing file without directory")
		}
	})

	t.Run("StatFile", func(t *testing.T) {
		testPath := "/test/stat.txt"
		testData := []byte("stat test")

		// Create parent directory
		err := fs.MkdirAll(filepath.Dir(testPath), 0755)
		if err != nil {
			t.Fatalf("Failed to create directory: %v", err)
		}

		// Write file
		err = fs.WriteFile(testPath, testData, 0644)
		if err != nil {
			t.Fatalf("Failed to write file: %v", err)
		}

		// Stat file
		info, err := fs.Stat(testPath)
		if err != nil {
			t.Fatalf("Failed to stat file: %v", err)
		}

		if info.IsDir() {
			t.Error("Expected file, got directory")
		}

		if info.Size() != int64(len(testData)) {
			t.Errorf("Expected size %d, got %d", len(testData), info.Size())
		}
	})

	t.Run("StatDirectory", func(t *testing.T) {
		testDir := "/test/dir"

		err := fs.MkdirAll(testDir, 0755)
		if err != nil {
			t.Fatalf("Failed to create directory: %v", err)
		}

		info, err := fs.Stat(testDir)
		if err != nil {
			t.Fatalf("Failed to stat directory: %v", err)
		}

		if !info.IsDir() {
			t.Error("Expected directory, got file")
		}
	})

	t.Run("RemoveFile", func(t *testing.T) {
		testPath := "/test/remove.txt"

		// Create parent directory
		err := fs.MkdirAll(filepath.Dir(testPath), 0755)
		if err != nil {
			t.Fatalf("Failed to create directory: %v", err)
		}

		// Write file
		err = fs.WriteFile(testPath, []byte("remove me"), 0644)
		if err != nil {
			t.Fatalf("Failed to write file: %v", err)
		}

		// Remove file
		err = fs.Remove(testPath)
		if err != nil {
			t.Fatalf("Failed to remove file: %v", err)
		}

		// Verify file is gone
		_, err = fs.ReadFile(testPath)
		if err == nil {
			t.Error("Expected error reading removed file")
		}
	})

	t.Run("Reset", func(t *testing.T) {
		// Add some files
		if err := fs.MkdirAll("/reset/test", 0755); err != nil {
			t.Fatalf("Failed to create directory: %v", err)
		}
		if err := fs.WriteFile("/reset/test/file.txt", []byte("test"), 0644); err != nil {
			t.Fatalf("Failed to write file: %v", err)
		}

		// Reset
		fs.Reset()

		// Verify everything is gone
		_, err := fs.ReadFile("/reset/test/file.txt")
		if err == nil {
			t.Error("Expected error after reset")
		}

		files := fs.GetAllFiles()
		if len(files) != 0 {
			t.Errorf("Expected 0 files after reset, got %d", len(files))
		}
	})

	t.Run("DataIsolation", func(t *testing.T) {
		testPath := "/test/isolation.txt"
		originalData := []byte("original")

		// Create parent directory
		err := fs.MkdirAll(filepath.Dir(testPath), 0755)
		if err != nil {
			t.Fatalf("Failed to create directory: %v", err)
		}

		// Write file
		err = fs.WriteFile(testPath, originalData, 0644)
		if err != nil {
			t.Fatalf("Failed to write file: %v", err)
		}

		// Read and modify the data
		data, _ := fs.ReadFile(testPath)
		data[0] = 'X' // Modify the returned slice

		// Read again to ensure original is unchanged
		data2, _ := fs.ReadFile(testPath)
		if string(data2) != string(originalData) {
			t.Error("File data was modified externally")
		}
	})
}

func TestOSFileSystem(t *testing.T) {
	// Test only basic functionality to ensure interface compliance
	fs := NewOSFileSystem()
	tmpDir := t.TempDir()

	t.Run("BasicOperations", func(t *testing.T) {
		testPath := fs.Join(tmpDir, "test.txt")
		testData := []byte("OS filesystem test")

		// Write file
		err := fs.WriteFile(testPath, testData, 0644)
		if err != nil {
			t.Fatalf("Failed to write file: %v", err)
		}

		// Read file
		data, err := fs.ReadFile(testPath)
		if err != nil {
			t.Fatalf("Failed to read file: %v", err)
		}

		if string(data) != string(testData) {
			t.Errorf("Expected %s, got %s", testData, data)
		}

		// Stat file
		info, err := fs.Stat(testPath)
		if err != nil {
			t.Fatalf("Failed to stat file: %v", err)
		}

		if info.IsDir() {
			t.Error("Expected file, got directory")
		}

		// Remove file
		err = fs.Remove(testPath)
		if err != nil {
			t.Fatalf("Failed to remove file: %v", err)
		}
	})
}
