package v2

import (
	"testing"
	"time"

	"github.com/arthur-debert/nanostore/nanostore"
)

func TestDebugCreate(t *testing.T) {
	tmpDir := t.TempDir()
	store, err := NewSimpleStore(tmpDir)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}
	defer store.Close()

	// Test different ways of creating PadzScratch
	tests := []struct {
		name   string
		create func() *PadzScratch
	}{
		{
			name: "Empty struct",
			create: func() *PadzScratch {
				return &PadzScratch{}
			},
		},
		{
			name: "With title only",
			create: func() *PadzScratch {
				return &PadzScratch{
					Document: nanostore.Document{
						Title: "Title Only",
					},
				}
			},
		},
		{
			name: "With timestamps",
			create: func() *PadzScratch {
				return &PadzScratch{
					Document: nanostore.Document{
						Title:     "With Timestamps",
						CreatedAt: time.Now(),
						UpdatedAt: time.Now(),
					},
				}
			},
		},
		{
			name: "FromScratch conversion",
			create: func() *PadzScratch {
				s := Scratch{
					ID:        "test-id",
					Title:     "From Scratch",
					CreatedAt: time.Now(),
					UpdatedAt: time.Now(),
				}
				return FromScratch(s)
			},
		},
		{
			name: "FromScratch without ID",
			create: func() *PadzScratch {
				s := Scratch{
					Title:     "From Scratch No ID",
					CreatedAt: time.Now(),
					UpdatedAt: time.Now(),
				}
				return FromScratch(s)
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			ps := tt.create()

			// Log the PadzScratch state before create
			t.Logf("Before create: UUID=%s, Title=%s, CreatedAt=%v",
				ps.UUID, ps.Title, ps.CreatedAt)

			// Create via API
			id, err := store.store.Create(ps.Title, ps)
			if err != nil {
				t.Errorf("Failed to create: %v", err)
				return
			}

			t.Logf("Created with ID: %s", id)

			// Query to see the result
			items, err := store.store.Query().Find()
			if err != nil {
				t.Errorf("Failed to query: %v", err)
				return
			}

			// Find our item
			for _, item := range items {
				if item.UUID == id {
					t.Logf("Result: UUID=%s, SimpleID=%s, Title=%s",
						item.UUID, item.SimpleID, item.Title)
					if item.SimpleID == item.UUID {
						t.Logf("  WARNING: SimpleID equals UUID!")
					}
					break
				}
			}
		})
	}
}
