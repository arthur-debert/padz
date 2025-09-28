package v2

import (
	"os"
	"testing"

	"github.com/arthur-debert/nanostore/nanostore"
	"github.com/arthur-debert/nanostore/nanostore/api"
)

// TestModel with different dimension configurations
type TestModel1 struct {
	nanostore.Document
	Status string `values:"active,done" default:"active"`
}

type TestModel2 struct {
	nanostore.Document
	Activity string `values:"active,deleted" default:"active"`
}

type TestModel3 struct {
	nanostore.Document
	Activity string `values:"active,archived,deleted" default:"active"`
	Status   string `values:"pending,done" default:"pending"`
}

func TestIDGenerationPatterns(t *testing.T) {
	tests := []struct {
		name   string
		create func(t *testing.T) (interface{}, func())
	}{
		{
			name: "SingleDimensionTwoValues",
			create: func(t *testing.T) (interface{}, func()) {
				tmpfile, _ := os.CreateTemp("", "test1*.json")
				store, _ := api.NewFromType[TestModel1](tmpfile.Name())
				cleanup := func() {
					store.Close()
					os.Remove(tmpfile.Name())
				}
				return store, cleanup
			},
		},
		{
			name: "SingleDimensionActivityOnly",
			create: func(t *testing.T) (interface{}, func()) {
				tmpfile, _ := os.CreateTemp("", "test2*.json")
				store, _ := api.NewFromType[TestModel2](tmpfile.Name())
				cleanup := func() {
					store.Close()
					os.Remove(tmpfile.Name())
				}
				return store, cleanup
			},
		},
		{
			name: "MultipleDimensions",
			create: func(t *testing.T) (interface{}, func()) {
				tmpfile, _ := os.CreateTemp("", "test3*.json")
				store, _ := api.NewFromType[TestModel3](tmpfile.Name())
				cleanup := func() {
					store.Close()
					os.Remove(tmpfile.Name())
				}
				return store, cleanup
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			storeInterface, cleanup := tt.create(t)
			defer cleanup()

			switch store := storeInterface.(type) {
			case *api.TypedStore[TestModel1]:
				// Create items
				id1, _ := store.Create("Item 1", &TestModel1{})
				id2, _ := store.Create("Item 2", &TestModel1{})
				t.Logf("Created IDs: %s, %s", id1, id2)

				// Query and check SimpleIDs
				items, _ := store.Query().Find()
				t.Logf("Found %d items:", len(items))
				for i, item := range items {
					t.Logf("  [%d] UUID=%s, SimpleID=%s, Title=%s",
						i, item.UUID, item.SimpleID, item.Title)
					if item.SimpleID == item.UUID {
						t.Logf("    WARNING: SimpleID equals UUID")
					}
				}

			case *api.TypedStore[TestModel2]:
				// Create items
				id1, _ := store.Create("Item 1", &TestModel2{})
				id2, _ := store.Create("Item 2", &TestModel2{})
				t.Logf("Created IDs: %s, %s", id1, id2)

				// Query and check SimpleIDs
				items, _ := store.Query().Find()
				t.Logf("Found %d items:", len(items))
				for i, item := range items {
					t.Logf("  [%d] UUID=%s, SimpleID=%s, Title=%s",
						i, item.UUID, item.SimpleID, item.Title)
					if item.SimpleID == item.UUID {
						t.Logf("    WARNING: SimpleID equals UUID")
					}
				}

			case *api.TypedStore[TestModel3]:
				// Create items
				id1, _ := store.Create("Item 1", &TestModel3{})
				id2, _ := store.Create("Item 2", &TestModel3{})
				t.Logf("Created IDs: %s, %s", id1, id2)

				// Query and check SimpleIDs
				items, _ := store.Query().Find()
				t.Logf("Found %d items:", len(items))
				for i, item := range items {
					t.Logf("  [%d] UUID=%s, SimpleID=%s, Title=%s",
						i, item.UUID, item.SimpleID, item.Title)
					if item.SimpleID == item.UUID {
						t.Logf("    WARNING: SimpleID equals UUID")
					}
				}
			}
		})
	}
}

// Test with our actual PadzScratch model
func TestPadzScratchIDGeneration(t *testing.T) {
	tmpfile, _ := os.CreateTemp("", "padz*.json")
	defer os.Remove(tmpfile.Name())

	store, err := api.NewFromType[PadzScratch](tmpfile.Name())
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}
	defer store.Close()

	// Create some scratches
	id1, _ := store.Create("Scratch 1", &PadzScratch{})
	id2, _ := store.Create("Scratch 2", &PadzScratch{})
	id3, _ := store.Create("Scratch 3", &PadzScratch{
		Pinned: "yes",
	})

	t.Logf("Created IDs: %s, %s, %s", id1, id2, id3)

	// Query all
	items, _ := store.Query().Find()
	t.Logf("\nAll items (%d):", len(items))
	for i, item := range items {
		t.Logf("  [%d] UUID=%s, SimpleID=%s, Title=%s, Pinned=%s",
			i, item.UUID, item.SimpleID, item.Title, item.Pinned)
	}

	// Query only Activity=active (should be all)
	activeItems, _ := store.Query().Activity("active").Find()
	t.Logf("\nActive items (%d):", len(activeItems))
	for i, item := range activeItems {
		t.Logf("  [%d] UUID=%s, SimpleID=%s, Title=%s",
			i, item.UUID, item.SimpleID, item.Title)
	}

	// Check if prefixes work for pinned items
	t.Log("\nChecking pinned items...")
	for _, item := range items {
		if item.Pinned == "yes" {
			t.Logf("Pinned item: SimpleID=%s (should start with 'p')", item.SimpleID)
		}
	}
}
