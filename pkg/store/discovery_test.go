package store

import (
	"crypto/sha1"
	"fmt"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestDiscoveryManager_DiscoverOrphanedFiles(t *testing.T) {
	cfg := &config.Config{
		FileSystem: filesystem.NewMemoryFileSystem(),
		DataPath:   "/tmp/test",
	}

	store, err := NewStoreWithConfig(cfg)
	require.NoError(t, err)

	// Enable new metadata system
	store.useNewMetadata = true
	require.NoError(t, store.metadataManager.Initialize())

	// Create some orphaned files directly
	filesPath := store.metadataManager.GetFilesPath()
	require.NoError(t, store.fs.MkdirAll(filesPath, 0755))

	// Create orphaned files
	content1 := []byte("This is orphaned content 1\nWith multiple lines")
	content2 := []byte("Another orphaned file")
	content3 := []byte("") // Empty file

	id1 := fmt.Sprintf("%x", sha1.Sum(content1))
	id2 := fmt.Sprintf("%x", sha1.Sum(content2))
	id3 := fmt.Sprintf("%x", sha1.Sum(content3))

	require.NoError(t, store.fs.WriteFile(store.fs.Join(filesPath, id1), content1, 0644))
	require.NoError(t, store.fs.WriteFile(store.fs.Join(filesPath, id2), content2, 0644))
	require.NoError(t, store.fs.WriteFile(store.fs.Join(filesPath, id3), content3, 0644))

	// Create one legitimate scratch with metadata
	legitContent := []byte("This has metadata")
	legitID := fmt.Sprintf("%x", sha1.Sum(legitContent))
	require.NoError(t, store.fs.WriteFile(store.fs.Join(filesPath, legitID), legitContent, 0644))

	legitScratch := &Scratch{
		ID:        legitID,
		Project:   "test",
		Title:     "Legitimate scratch",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	require.NoError(t, store.metadataManager.SaveScratchMetadata(legitScratch))
	require.NoError(t, store.metadataManager.UpdateIndexEntry(legitScratch))

	// Run discovery
	dm := NewDiscoveryManager(store)
	err = dm.DiscoverOrphanedFiles()
	require.NoError(t, err)

	// Verify results
	// Load index to check
	index, err := store.metadataManager.LoadIndex()
	require.NoError(t, err)

	// Should have 4 total entries (3 orphaned + 1 legitimate)
	assert.Len(t, index.Scratches, 4)

	// Check orphaned files were added
	assert.Contains(t, index.Scratches, id1)
	assert.Contains(t, index.Scratches, id2)
	assert.Contains(t, index.Scratches, id3)
	assert.Contains(t, index.Scratches, legitID)

	// Check metadata for orphaned files
	orphan1, err := store.metadataManager.LoadScratchMetadata(id1)
	require.NoError(t, err)
	assert.Equal(t, "recovered", orphan1.Project)
	assert.Equal(t, "This is orphaned content 1", orphan1.Title)
	assert.Equal(t, int64(len(content1)), orphan1.Size)

	orphan2, err := store.metadataManager.LoadScratchMetadata(id2)
	require.NoError(t, err)
	assert.Equal(t, "recovered", orphan2.Project)
	assert.Equal(t, "Another orphaned file", orphan2.Title)

	orphan3, err := store.metadataManager.LoadScratchMetadata(id3)
	require.NoError(t, err)
	assert.Equal(t, "recovered", orphan3.Project)
	assert.Equal(t, "Untitled (recovered)", orphan3.Title) // Empty file gets default title
}

func TestDiscoveryManager_ExtractTitle(t *testing.T) {
	dm := &DiscoveryManager{}

	tests := []struct {
		name     string
		content  []byte
		expected string
	}{
		{
			name:     "first line as title",
			content:  []byte("This is the title\nThis is the body"),
			expected: "This is the title",
		},
		{
			name:     "skip empty lines",
			content:  []byte("\n\n\nActual title\nBody"),
			expected: "Actual title",
		},
		{
			name:     "long title truncated",
			content:  []byte("This is a very long title that exceeds the maximum length limit and should be truncated with ellipsis at the end to fit within 100 characters"),
			expected: "This is a very long title that exceeds the maximum length limit and should be truncated with elli...",
		},
		{
			name:     "empty content",
			content:  []byte(""),
			expected: "Untitled (recovered)",
		},
		{
			name:     "only whitespace",
			content:  []byte("   \n\t\n   "),
			expected: "Untitled (recovered)",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := dm.extractTitle(tt.content)
			assert.Equal(t, tt.expected, result)
		})
	}
}

func TestStore_RunDiscoveryBeforeCommand(t *testing.T) {
	cfg := &config.Config{
		FileSystem: filesystem.NewMemoryFileSystem(),
		DataPath:   "/tmp/test",
	}

	store, err := NewStoreWithConfig(cfg)
	require.NoError(t, err)

	// Test with old metadata system (should be no-op)
	store.useNewMetadata = false
	err = store.RunDiscoveryBeforeCommand()
	assert.NoError(t, err)

	// Enable new metadata system
	store.useNewMetadata = true
	require.NoError(t, store.metadataManager.Initialize())

	// Create an orphaned file
	filesPath := store.metadataManager.GetFilesPath()
	require.NoError(t, store.fs.MkdirAll(filesPath, 0755))

	content := []byte("Orphaned content")
	id := fmt.Sprintf("%x", sha1.Sum(content))
	require.NoError(t, store.fs.WriteFile(store.fs.Join(filesPath, id), content, 0644))

	// Run discovery
	err = store.RunDiscoveryBeforeCommand()
	require.NoError(t, err)

	// Verify the file was discovered
	index, err := store.metadataManager.LoadIndex()
	require.NoError(t, err)
	assert.Contains(t, index.Scratches, id)
}

func TestDiscoveryManager_IdempotentDiscovery(t *testing.T) {
	cfg := &config.Config{
		FileSystem: filesystem.NewMemoryFileSystem(),
		DataPath:   "/tmp/test",
	}

	store, err := NewStoreWithConfig(cfg)
	require.NoError(t, err)

	store.useNewMetadata = true
	require.NoError(t, store.metadataManager.Initialize())

	// Create an orphaned file
	filesPath := store.metadataManager.GetFilesPath()
	require.NoError(t, store.fs.MkdirAll(filesPath, 0755))

	content := []byte("Test content")
	id := fmt.Sprintf("%x", sha1.Sum(content))
	require.NoError(t, store.fs.WriteFile(store.fs.Join(filesPath, id), content, 0644))

	dm := NewDiscoveryManager(store)

	// Run discovery first time
	err = dm.DiscoverOrphanedFiles()
	require.NoError(t, err)

	// Verify it was discovered
	index, err := store.metadataManager.LoadIndex()
	require.NoError(t, err)
	assert.Len(t, index.Scratches, 1)

	// Run discovery multiple more times
	for i := 0; i < 3; i++ {
		err = dm.DiscoverOrphanedFiles()
		require.NoError(t, err)
	}

	// Should still have only one entry
	index, err = store.metadataManager.LoadIndex()
	require.NoError(t, err)
	assert.Len(t, index.Scratches, 1)

	// Check that metadata exists
	metadata, err := store.metadataManager.LoadScratchMetadata(id)
	require.NoError(t, err)
	assert.NotNil(t, metadata)
	assert.Equal(t, "Test content", metadata.Title)
}
