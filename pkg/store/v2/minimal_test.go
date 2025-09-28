package v2

import (
	"os"
	"testing"

	"github.com/arthur-debert/nanostore/nanostore"
	"github.com/arthur-debert/nanostore/nanostore/api"
)

// MinimalTodo exactly matches the sample
type MinimalTodo struct {
	nanostore.Document

	Status   string `values:"pending,active,done" prefix:"done=d" default:"pending"`
	Priority string `values:"low,medium,high" prefix:"high=h" default:"medium"`
	Activity string `values:"active,archived,deleted" default:"active"`
	ParentID string `dimension:"parent_id,ref"`
}

func TestMinimalNanostore(t *testing.T) {
	// Create temp file
	tmpfile, err := os.CreateTemp("", "minimal*.json")
	if err != nil {
		t.Fatal(err)
	}
	defer os.Remove(tmpfile.Name())
	tmpfile.Close()

	// Create store
	store, err := api.NewFromType[MinimalTodo](tmpfile.Name())
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}
	defer store.Close()

	// Create first todo
	id1, err := store.Create("First Todo", &MinimalTodo{})
	if err != nil {
		t.Fatalf("Failed to create first todo: %v", err)
	}
	t.Logf("Created first todo with ID: %s", id1)

	// Create second todo
	id2, err := store.Create("Second Todo", &MinimalTodo{})
	if err != nil {
		t.Fatalf("Failed to create second todo: %v", err)
	}
	t.Logf("Created second todo with ID: %s", id2)

	// Query all todos
	todos, err := store.Query().Find()
	if err != nil {
		t.Fatalf("Failed to query todos: %v", err)
	}

	// Check results
	t.Logf("Found %d todos:", len(todos))
	for i, todo := range todos {
		t.Logf("  [%d] UUID=%s, SimpleID=%s, Title=%s",
			i, todo.UUID, todo.SimpleID, todo.Title)

		// Verify SimpleID is not UUID
		if todo.SimpleID == todo.UUID {
			t.Errorf("SimpleID should not equal UUID, but both are: %s", todo.UUID)
		}
	}

	// Test hierarchical todos
	id3, err := store.Create("Subtask of First", &MinimalTodo{
		ParentID: id1,
	})
	if err != nil {
		t.Fatalf("Failed to create subtask: %v", err)
	}
	t.Logf("Created subtask with ID: %s", id3)

	// Query again
	todos, err = store.Query().Find()
	if err != nil {
		t.Fatalf("Failed to query todos after subtask: %v", err)
	}

	t.Logf("After adding subtask, found %d todos:", len(todos))
	for i, todo := range todos {
		t.Logf("  [%d] UUID=%s, SimpleID=%s, Title=%s, ParentID=%s",
			i, todo.UUID, todo.SimpleID, todo.Title, todo.ParentID)
	}
}
