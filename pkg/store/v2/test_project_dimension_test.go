package v2

import (
	"testing"

	"github.com/arthur-debert/nanostore/nanostore"
	"github.com/stretchr/testify/require"
)

func TestProjectDimension(t *testing.T) {
	tmpDir := t.TempDir()
	dbPath := tmpDir + "/test-project.json"
	
	// Test with project as hierarchical
	config := nanostore.Config{
		Dimensions: []nanostore.DimensionConfig{
			{
				Name:         "activity",
				Type:         nanostore.Enumerated,
				Values:       []string{"active", "deleted"},
				Prefixes:     map[string]string{},
				DefaultValue: "active",
			},
			{
				Name:     "project",
				Type:     nanostore.Hierarchical,
				RefField: "project",
			},
		},
	}
	
	store, err := nanostore.New(dbPath, config)
	require.NoError(t, err)
	defer func() {
		_ = store.Close()
	}()
	
	// Test 1: Add with empty project
	t.Run("EmptyProject", func(t *testing.T) {
		uuid, err := store.Add("Empty project", map[string]interface{}{
			"activity": "active",
			"project":  "",
		})
		require.NoError(t, err)
		t.Logf("Created with empty project: %s", uuid)
		
		docs, err := store.List(nanostore.ListOptions{})
		require.NoError(t, err)
		require.Greater(t, len(docs), 0)
		
		doc := docs[0]
		t.Logf("Doc: UUID=%s, SimpleID=%s, Project=%v", doc.UUID, doc.SimpleID, doc.Dimensions["project"])
	})
	
	// Test 2: Add with project value
	t.Run("WithProject", func(t *testing.T) {
		uuid, err := store.Add("With project", map[string]interface{}{
			"activity": "active",
			"project":  "myproject",
		})
		require.NoError(t, err)
		t.Logf("Created with project: %s", uuid)
		
		docs, err := store.List(nanostore.ListOptions{})
		require.NoError(t, err)
		
		for i, doc := range docs {
			t.Logf("Doc %d: UUID=%s, SimpleID=%s, Project=%v", 
				i, doc.UUID, doc.SimpleID, doc.Dimensions["project"])
		}
	})
}

func TestProjectAsEnumerated(t *testing.T) {
	tmpDir := t.TempDir()
	dbPath := tmpDir + "/test-enum-project.json"
	
	// Try project as enumerated instead
	config := nanostore.Config{
		Dimensions: []nanostore.DimensionConfig{
			{
				Name:         "activity",
				Type:         nanostore.Enumerated,
				Values:       []string{"active", "deleted"},
				Prefixes:     map[string]string{},
				DefaultValue: "active",
			},
			{
				Name:         "project",
				Type:         nanostore.Enumerated,
				Values:       []string{"default", "work", "personal"},
				Prefixes:     map[string]string{},
				DefaultValue: "default",
			},
		},
	}
	
	store, err := nanostore.New(dbPath, config)
	require.NoError(t, err)
	defer func() {
		_ = store.Close()
	}()
	
	// Add documents
	uuid1, err := store.Add("First", map[string]interface{}{
		"activity": "active",
		"project":  "default",
	})
	require.NoError(t, err)
	t.Logf("Created: %s", uuid1)
	
	// List
	docs, err := store.List(nanostore.ListOptions{})
	require.NoError(t, err)
	
	for _, doc := range docs {
		t.Logf("Doc: UUID=%s, SimpleID=%s, Project=%v", 
			doc.UUID, doc.SimpleID, doc.Dimensions["project"])
	}
	
	require.Equal(t, "1", docs[0].SimpleID)
}