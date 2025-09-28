package v2

import (
	"testing"

	"github.com/arthur-debert/nanostore/nanostore"
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/stretchr/testify/require"
)

func TestDebugSimpleID(t *testing.T) {
	tmpDir := t.TempDir()
	
	// Test 1: Create store exactly like too app
	t.Run("MinimalNanostore", func(t *testing.T) {
		dbPath := tmpDir + "/test1.json"
		
		// Use exact same config as too app
		config := nanostore.Config{
			Dimensions: []nanostore.DimensionConfig{
				{
					Name:         "status",
					Type:         nanostore.Enumerated,
					Values:       []string{"pending", "completed"},
					Prefixes:     map[string]string{"completed": "c"},
					DefaultValue: "pending",
				},
			},
		}
		
		store, err := nanostore.New(dbPath, config)
		require.NoError(t, err)
		defer func() {
			_ = store.Close()
		}()
		
		// Add a document like too app
		uuid, err := store.Add("Test todo", map[string]interface{}{
			"status": "pending",
		})
		require.NoError(t, err)
		t.Logf("Created document with UUID: %s", uuid)
		
		// List documents
		docs, err := store.List(nanostore.ListOptions{
			Filters: map[string]interface{}{
				"status": "pending",
			},
		})
		require.NoError(t, err)
		require.Len(t, docs, 1)
		
		// Check SimpleID
		doc := docs[0]
		t.Logf("Document: UUID=%s, SimpleID=%s, Title=%s", doc.UUID, doc.SimpleID, doc.Title)
		require.Equal(t, "1", doc.SimpleID, "SimpleID should be '1' for first document")
	})
	
	// Test 2: Try our padz config
	t.Run("PadzConfig", func(t *testing.T) {
		dbPath := tmpDir + "/test2.json"
		
		config := nanostore.Config{
			Dimensions: []nanostore.DimensionConfig{
				{
					Name:         "activity",
					Type:         nanostore.Enumerated,
					Values:       []string{"active", "deleted"},
					Prefixes:     map[string]string{}, // No prefix for activity
					DefaultValue: "active",
				},
				{
					Name:         "pinned",
					Type:         nanostore.Enumerated,
					Values:       []string{"no", "yes"},
					Prefixes:     map[string]string{"yes": "p"},
					DefaultValue: "no",
				},
			},
		}
		
		store, err := nanostore.New(dbPath, config)
		require.NoError(t, err)
		defer func() {
			_ = store.Close()
		}()
		
		// Add documents
		uuid1, err := store.Add("First scratch", map[string]interface{}{
			"activity": "active",
			"pinned":   "no",
		})
		require.NoError(t, err)
		t.Logf("Created document 1 with UUID: %s", uuid1)
		
		uuid2, err := store.Add("Second scratch", map[string]interface{}{
			"activity": "active",
			"pinned":   "yes",
		})
		require.NoError(t, err)
		t.Logf("Created document 2 with UUID: %s", uuid2)
		
		// List all active documents
		docs, err := store.List(nanostore.ListOptions{
			Filters: map[string]interface{}{
				"activity": "active",
			},
		})
		require.NoError(t, err)
		require.Len(t, docs, 2)
		
		// Check SimpleIDs
		for i, doc := range docs {
			t.Logf("Doc %d: UUID=%s, SimpleID=%s, Title=%s, Pinned=%v", 
				i, doc.UUID, doc.SimpleID, doc.Title, doc.Dimensions["pinned"])
		}
		
		// Find the pinned one
		var pinnedID, unpinnedID string
		for _, doc := range docs {
			if doc.Dimensions["pinned"] == "yes" {
				pinnedID = doc.SimpleID
			} else {
				unpinnedID = doc.SimpleID
			}
		}
		
		require.Equal(t, "p1", pinnedID, "Pinned item should have SimpleID 'p1'")
		require.Equal(t, "1", unpinnedID, "Unpinned item should have SimpleID '1'")
	})
}

func TestOurStoreSimpleID(t *testing.T) {
	// Test our NanoStore implementation
	tmpDir := t.TempDir()
	fs := filesystem.NewOSFileSystem()
	cfg := &config.Config{
		FileSystem:    fs,
		DataPath:      tmpDir,
		IsGlobalScope: false,
	}

	store, err := NewNanoStoreWithConfig(cfg)
	require.NoError(t, err)
	defer func() {
		if err := store.Close(); err != nil {
			t.Errorf("Failed to close store: %v", err)
		}
	}()

	// Add scratches
	err = store.AddScratch(Scratch{
		Title:   "First",
		Project: "test",
	})
	require.NoError(t, err)

	// Get scratches and debug
	scratches := store.GetScratches()
	require.Len(t, scratches, 1)
	
	t.Logf("Scratch: ID=%s, Title=%s", scratches[0].ID, scratches[0].Title)
	
	// Try to access the underlying nanostore directly
	// This would help us debug if the issue is in our wrapper
	t.Log("DEBUG: Need to check what store.List() returns directly")
}

func TestNanoStoreWithoutProject(t *testing.T) {
	tmpDir := t.TempDir()
	dbPath := tmpDir + "/test-no-project.json"
	
	// Create config without project dimension
	config := nanostore.Config{
		Dimensions: []nanostore.DimensionConfig{
			{
				Name:         "activity",
				Type:         nanostore.Enumerated,
				Values:       []string{"active", "deleted"},
				Prefixes:     map[string]string{}, // No prefix for activity
				DefaultValue: "active",
			},
			{
				Name:         "pinned",
				Type:         nanostore.Enumerated,
				Values:       []string{"no", "yes"},
				Prefixes:     map[string]string{"yes": "p"},
				DefaultValue: "no",
			},
		},
	}
	
	store, err := nanostore.New(dbPath, config)
	require.NoError(t, err)
	defer func() {
		_ = store.Close()
	}()
	
	// Add a document
	uuid, err := store.Add("Test scratch", map[string]interface{}{
		"activity": "active",
		"pinned":   "no",
	})
	require.NoError(t, err)
	t.Logf("Created document with UUID: %s", uuid)
	
	// List documents
	docs, err := store.List(nanostore.ListOptions{
		Filters: map[string]interface{}{
			"activity": "active",
		},
	})
	require.NoError(t, err)
	require.Len(t, docs, 1)
	
	doc := docs[0]
	t.Logf("Document: UUID=%s, SimpleID=%s, Title=%s", doc.UUID, doc.SimpleID, doc.Title)
	require.Equal(t, "1", doc.SimpleID, "SimpleID should be '1' for first document")
}