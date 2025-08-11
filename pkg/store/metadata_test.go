package store

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestMetadataManager(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "metadata_test")
	require.NoError(t, err)
	defer func() { _ = os.RemoveAll(tmpDir) }()

	fs := filesystem.NewOSFileSystem()
	mm := NewMetadataManager(fs, tmpDir)

	t.Run("Initialize", func(t *testing.T) {
		err := mm.Initialize()
		assert.NoError(t, err)

		// Check directories exist
		assert.DirExists(t, mm.GetFilesPath())
		assert.DirExists(t, mm.GetMetadataPath())

		// Check empty index exists
		assert.FileExists(t, mm.GetIndexPath())

		index, err := mm.LoadIndex()
		assert.NoError(t, err)
		assert.Equal(t, indexVersion, index.Version)
		assert.Empty(t, index.Scratches)
	})

	t.Run("SaveAndLoadScratchMetadata", func(t *testing.T) {
		scratch := &Scratch{
			ID:        "test123",
			Project:   "test-project",
			Title:     "Test Scratch",
			CreatedAt: time.Now(),
			UpdatedAt: time.Now(),
			Size:      100,
			Checksum:  "sha256:abc123",
		}

		// Save metadata
		err := mm.SaveScratchMetadata(scratch)
		assert.NoError(t, err)

		// Load metadata
		loaded, err := mm.LoadScratchMetadata(scratch.ID)
		assert.NoError(t, err)
		assert.NotNil(t, loaded)
		assert.Equal(t, scratch.ID, loaded.ID)
		assert.Equal(t, scratch.Title, loaded.Title)
		assert.Equal(t, scratch.Project, loaded.Project)
	})

	t.Run("UpdateIndex", func(t *testing.T) {
		scratch := &Scratch{
			ID:        "index123",
			Project:   "index-project",
			Title:     "Index Test",
			CreatedAt: time.Now(),
		}

		// Update index
		err := mm.UpdateIndexEntry(scratch)
		assert.NoError(t, err)

		// Load index
		index, err := mm.LoadIndex()
		assert.NoError(t, err)
		assert.Len(t, index.Scratches, 1)

		entry, exists := index.Scratches[scratch.ID]
		assert.True(t, exists)
		assert.Equal(t, scratch.Project, entry.Project)
		assert.Equal(t, scratch.Title, entry.Title)
	})

	t.Run("RemoveFromIndex", func(t *testing.T) {
		// First add
		scratch := &Scratch{
			ID:        "remove123",
			Project:   "remove-project",
			Title:     "Remove Test",
			CreatedAt: time.Now(),
		}
		err := mm.UpdateIndexEntry(scratch)
		assert.NoError(t, err)

		// Then remove
		err = mm.RemoveIndexEntry(scratch.ID)
		assert.NoError(t, err)

		// Verify removed
		index, err := mm.LoadIndex()
		assert.NoError(t, err)
		_, exists := index.Scratches[scratch.ID]
		assert.False(t, exists)
	})

	t.Run("RebuildIndex", func(t *testing.T) {
		// Create new temp dir for rebuild test
		rebuildTmpDir, err := os.MkdirTemp("", "rebuild_test")
		require.NoError(t, err)
		defer func() { _ = os.RemoveAll(rebuildTmpDir) }()

		rebuildMM := NewMetadataManager(fs, rebuildTmpDir)

		// Initialize first to create directories
		err = rebuildMM.Initialize()
		assert.NoError(t, err)

		// Create some metadata files
		scratches := []Scratch{
			{ID: "rebuild1", Project: "proj1", Title: "Title 1", CreatedAt: time.Now()},
			{ID: "rebuild2", Project: "proj2", Title: "Title 2", CreatedAt: time.Now()},
			{ID: "rebuild3", Project: "proj3", Title: "Title 3", CreatedAt: time.Now()},
		}

		for _, s := range scratches {
			s := s // capture range variable
			err := rebuildMM.SaveScratchMetadata(&s)
			assert.NoError(t, err)
		}

		// Corrupt the index by clearing it
		err = rebuildMM.SaveIndex(&Index{
			Version:   indexVersion,
			UpdatedAt: time.Now(),
			Scratches: make(map[string]IndexEntry),
		})
		assert.NoError(t, err)

		// Rebuild
		err = rebuildMM.RebuildIndex()
		assert.NoError(t, err)

		// Verify all entries are back
		index, err := rebuildMM.LoadIndex()
		assert.NoError(t, err)
		assert.Len(t, index.Scratches, 3)
		for _, s := range scratches {
			entry, exists := index.Scratches[s.ID]
			assert.True(t, exists)
			assert.Equal(t, s.Project, entry.Project)
			assert.Equal(t, s.Title, entry.Title)
		}
	})

	t.Run("MigrateFromLegacy", func(t *testing.T) {
		// Create new temp dir for migration test
		migrateTmpDir, err := os.MkdirTemp("", "migrate_test")
		require.NoError(t, err)
		defer func() { _ = os.RemoveAll(migrateTmpDir) }()

		migrateMM := NewMetadataManager(fs, migrateTmpDir)

		// Create legacy scratches
		legacyScratches := []Scratch{
			{ID: "legacy1", Project: "proj1", Title: "Legacy 1", CreatedAt: time.Now()},
			{ID: "legacy2", Project: "proj2", Title: "Legacy 2", CreatedAt: time.Now()},
		}

		// Create legacy content files in root
		for _, s := range legacyScratches {
			content := []byte("content for " + s.ID)
			err := fs.WriteFile(filepath.Join(migrateTmpDir, s.ID), content, 0644)
			assert.NoError(t, err)
		}

		// Migrate
		err = migrateMM.MigrateFromLegacyMetadata(legacyScratches)
		assert.NoError(t, err)

		// Verify migration
		// 1. Check index
		index, err := migrateMM.LoadIndex()
		assert.NoError(t, err)
		assert.Len(t, index.Scratches, 2)

		// 2. Check individual metadata files
		for _, s := range legacyScratches {
			loaded, err := migrateMM.LoadScratchMetadata(s.ID)
			assert.NoError(t, err)
			assert.NotNil(t, loaded)
			assert.Equal(t, s.ID, loaded.ID)
			assert.Equal(t, s.Title, loaded.Title)
		}

		// 3. Check content files moved to files/
		for _, s := range legacyScratches {
			newPath := filepath.Join(migrateMM.GetFilesPath(), s.ID)
			assert.FileExists(t, newPath)

			oldPath := filepath.Join(migrateTmpDir, s.ID)
			_, err := os.Stat(oldPath)
			assert.True(t, os.IsNotExist(err), "Old file should be removed")
		}

		// 4. Check backup created
		backupPath := filepath.Join(migrateTmpDir, metadataFileName+".backup")
		if _, err := os.Stat(filepath.Join(migrateTmpDir, metadataFileName)); err == nil {
			assert.FileExists(t, backupPath)
		}
	})
}

func TestStoreWithNewMetadata(t *testing.T) {
	tmpDir, err := os.MkdirTemp("", "store_metadata_test")
	require.NoError(t, err)
	defer func() { _ = os.RemoveAll(tmpDir) }()

	cfg := &config.Config{
		FileSystem: filesystem.NewOSFileSystem(),
		DataPath:   tmpDir,
	}

	t.Run("NewStoreTriggersAutomaticMigration", func(t *testing.T) {
		// Create legacy metadata.json
		legacyScratches := []Scratch{
			{ID: "auto1", Project: "proj1", Title: "Auto 1", CreatedAt: time.Now()},
			{ID: "auto2", Project: "proj2", Title: "Auto 2", CreatedAt: time.Now()},
		}

		// Write legacy metadata
		metadataPath := filepath.Join(tmpDir, dataDirName, metadataFileName)
		err = cfg.FileSystem.MkdirAll(filepath.Join(tmpDir, dataDirName), 0755)
		require.NoError(t, err)

		data, err := json.Marshal(legacyScratches)
		require.NoError(t, err)
		err = cfg.FileSystem.WriteFile(metadataPath, data, 0644)
		require.NoError(t, err)

		// Create store - should trigger migration
		store, err := NewStoreWithConfig(cfg)
		assert.NoError(t, err)
		assert.NotNil(t, store)
		assert.True(t, store.useNewMetadata)

		// Verify migration happened
		scratches := store.GetScratches()
		assert.Len(t, scratches, 2)

		// Check new structure exists
		indexPath := store.metadataManager.GetIndexPath()
		assert.FileExists(t, indexPath)
	})

	t.Run("ConcurrentOperations", func(t *testing.T) {
		// Create new temp dir for concurrent test
		concurrentTmpDir, err := os.MkdirTemp("", "concurrent_test")
		require.NoError(t, err)
		defer func() { _ = os.RemoveAll(concurrentTmpDir) }()

		concurrentCfg := &config.Config{
			FileSystem: filesystem.NewOSFileSystem(),
			DataPath:   concurrentTmpDir,
		}

		store, err := NewStoreWithConfig(concurrentCfg)
		require.NoError(t, err)

		// Force new metadata system
		err = store.metadataManager.Initialize()
		require.NoError(t, err)
		store.useNewMetadata = true

		// Run concurrent adds
		done := make(chan bool, 10)
		for i := 0; i < 10; i++ {
			go func(n int) {
				scratch := Scratch{
					ID:        fmt.Sprintf("concurrent%d", n),
					Project:   "test",
					Title:     fmt.Sprintf("Concurrent %d", n),
					CreatedAt: time.Now(),
				}
				err := store.AddScratchAtomic(scratch)
				assert.NoError(t, err)
				done <- true
			}(i)
		}

		// Wait for all to complete
		for i := 0; i < 10; i++ {
			<-done
		}

		// Verify all added
		scratches := store.GetScratches()
		assert.Len(t, scratches, 10)

		// Verify index consistency
		index, err := store.metadataManager.LoadIndex()
		assert.NoError(t, err)
		assert.Len(t, index.Scratches, 10)
	})
}
