package commands

import (
	"fmt"
	"strings"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func createTestStore(t *testing.T) *store.Store {
	t.Helper()
	fs := filesystem.NewMemoryFileSystem()
	cfg := &config.Config{
		FileSystem: fs,
		DataPath:   "/test",
	}

	s, err := store.NewStoreWithConfig(cfg)
	require.NoError(t, err)
	return s
}

func createTestScratches(t *testing.T, s *store.Store, count int) []store.Scratch {
	t.Helper()
	scratches := make([]store.Scratch, count)

	// Create scratches with increasing creation times so we have predictable ordering
	// Scratch 0 is oldest, scratch count-1 is newest
	baseTime := time.Now().Add(-time.Duration(count) * time.Hour)

	for i := 0; i < count; i++ {
		scratch := store.Scratch{
			ID:        fmt.Sprintf("hash%d", i+1),
			Project:   "test",
			Title:     fmt.Sprintf("Test Scratch %d", i+1),
			CreatedAt: baseTime.Add(time.Duration(i) * time.Hour),
			UpdatedAt: baseTime.Add(time.Duration(i) * time.Hour),
		}

		// Make first two pinned (will be the two oldest)
		if i < 2 {
			scratch.IsPinned = true
			scratch.PinnedAt = scratch.CreatedAt
		}

		// Make last two deleted (will be the two newest)
		if i >= count-2 {
			scratch.IsDeleted = true
			// Deleted more recently than created
			deletedAt := scratch.CreatedAt.Add(30 * time.Minute)
			scratch.DeletedAt = &deletedAt
		}

		scratches[i] = scratch
		err := s.AddScratch(scratch)
		require.NoError(t, err)
	}

	return scratches
}

func TestResolveMultipleIDs(t *testing.T) {
	s := createTestStore(t)
	scratches := createTestScratches(t, s, 5)

	tests := []struct {
		name        string
		ids         []string
		expectError bool
		errorMsg    string
		expectCount int
		expectIDs   []string
	}{
		{
			name:        "empty slice",
			ids:         []string{},
			expectError: false,
			expectCount: 0,
		},
		{
			name:        "single regular index",
			ids:         []string{"1"},
			expectError: false,
			expectCount: 1,
			expectIDs:   []string{scratches[2].ID}, // Index 1 = newest non-deleted (scratch[2])
		},
		{
			name:        "multiple regular indices",
			ids:         []string{"1", "2", "3"},
			expectError: false,
			expectCount: 3,
			expectIDs:   []string{scratches[2].ID, scratches[1].ID, scratches[0].ID}, // Newest to oldest non-deleted
		},
		{
			name:        "pinned indices",
			ids:         []string{"p1", "p2"},
			expectError: false,
			expectCount: 2,
			expectIDs:   []string{scratches[1].ID, scratches[0].ID}, // Only first two are pinned, in sorted order
		},
		{
			name:        "deleted indices",
			ids:         []string{"d1", "d2"},
			expectError: false,
			expectCount: 2,
			expectIDs:   []string{scratches[4].ID, scratches[3].ID}, // Most recent deleted first
		},
		{
			name:        "hash prefixes",
			ids:         []string{"hash1", "hash2"},
			expectError: false,
			expectCount: 2,
			expectIDs:   []string{scratches[0].ID, scratches[1].ID},
		},
		{
			name:        "mixed ID types",
			ids:         []string{"1", "p1", "hash3"},
			expectError: false,
			expectCount: 3,
			expectIDs:   []string{scratches[2].ID, scratches[1].ID, scratches[2].ID}, // Index 1, pinned 1, and hash3
		},
		{
			name:        "duplicates handled gracefully",
			ids:         []string{"1", "1", "2", "2"},
			expectError: false,
			expectCount: 2,                                          // Each unique ID appears once
			expectIDs:   []string{scratches[2].ID, scratches[1].ID}, // Index 1 and Index 2
		},
		{
			name:        "invalid index",
			ids:         []string{"1", "999", "2"},
			expectError: true,
			errorMsg:    "failed to resolve IDs",
		},
		{
			name:        "invalid pinned index",
			ids:         []string{"p99"},
			expectError: true,
			errorMsg:    "failed to resolve IDs",
		},
		{
			name:        "invalid deleted index",
			ids:         []string{"d99"},
			expectError: true,
			errorMsg:    "failed to resolve IDs",
		},
		{
			name:        "invalid hash",
			ids:         []string{"notfound"},
			expectError: true,
			errorMsg:    "failed to resolve IDs",
		},
		{
			name:        "mixed valid and invalid",
			ids:         []string{"1", "invalid", "2"},
			expectError: true,
			errorMsg:    "failed to resolve IDs",
		},
		{
			name:        "empty strings ignored",
			ids:         []string{"1", "", "2"},
			expectError: false,
			expectCount: 2,
			expectIDs:   []string{scratches[2].ID, scratches[1].ID}, // Index 1 -> scratches[2], Index 2 -> scratches[1]
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results, err := ResolveMultipleIDs(s, false, false, "test", tt.ids)

			if tt.expectError {
				assert.Error(t, err)
				assert.Contains(t, err.Error(), tt.errorMsg)
				assert.Nil(t, results)
			} else {
				assert.NoError(t, err)
				assert.Len(t, results, tt.expectCount)

				if tt.expectIDs != nil {
					for i, result := range results {
						assert.Equal(t, tt.expectIDs[i], result.ID)
					}
				}
			}
		})
	}
}

func TestResolveMultipleIDsWithErrors(t *testing.T) {
	s := createTestStore(t)
	createTestScratches(t, s, 5)

	tests := []struct {
		name          string
		ids           []string
		expectResults int
		checkResults  func(t *testing.T, results []ResolveResult)
	}{
		{
			name:          "all valid",
			ids:           []string{"1", "2", "3"},
			expectResults: 3,
			checkResults: func(t *testing.T, results []ResolveResult) {
				for _, r := range results {
					assert.NoError(t, r.Error)
					assert.NotNil(t, r.Scratch)
				}
			},
		},
		{
			name:          "some invalid",
			ids:           []string{"1", "invalid", "2", "notfound"},
			expectResults: 4,
			checkResults: func(t *testing.T, results []ResolveResult) {
				assert.NoError(t, results[0].Error)
				assert.NotNil(t, results[0].Scratch)

				assert.Error(t, results[1].Error)
				assert.Nil(t, results[1].Scratch)

				assert.NoError(t, results[2].Error)
				assert.NotNil(t, results[2].Scratch)

				assert.Error(t, results[3].Error)
				assert.Nil(t, results[3].Scratch)
			},
		},
		{
			name:          "duplicates reference same scratch",
			ids:           []string{"1", "1", "2"},
			expectResults: 3, // All three IDs get results (duplicates included)
			checkResults: func(t *testing.T, results []ResolveResult) {
				// All should succeed
				for _, r := range results {
					assert.NoError(t, r.Error)
					assert.NotNil(t, r.Scratch)
				}

				// First two should reference the same scratch (both are "1")
				assert.Equal(t, results[0].Scratch.ID, results[1].Scratch.ID)
				assert.Equal(t, "1", results[0].ID)
				assert.Equal(t, "1", results[1].ID)

				// Third is different
				assert.Equal(t, "2", results[2].ID)
				assert.NotEqual(t, results[0].Scratch.ID, results[2].Scratch.ID)
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results := ResolveMultipleIDsWithErrors(s, false, false, "test", tt.ids)
			assert.Len(t, results, tt.expectResults)

			if tt.checkResults != nil {
				tt.checkResults(t, results)
			}
		})
	}
}

func TestValidateIDs(t *testing.T) {
	tests := []struct {
		name        string
		ids         []string
		expectError bool
		errorMsg    string
	}{
		{
			name:        "empty slice",
			ids:         []string{},
			expectError: false,
		},
		{
			name:        "valid regular indices",
			ids:         []string{"1", "2", "99", "1000"},
			expectError: false,
		},
		{
			name:        "valid pinned indices",
			ids:         []string{"p1", "p2", "p99"},
			expectError: false,
		},
		{
			name:        "valid deleted indices",
			ids:         []string{"d1", "d2", "d99"},
			expectError: false,
		},
		{
			name:        "valid hash prefixes",
			ids:         []string{"a", "abc", "123def", "ABCDEF123"},
			expectError: false,
		},
		{
			name:        "empty string",
			ids:         []string{""},
			expectError: true,
			errorMsg:    "(empty)",
		},
		{
			name:        "invalid regular index - zero",
			ids:         []string{"0"},
			expectError: true,
			errorMsg:    "index must be positive",
		},
		{
			name:        "invalid regular index - negative",
			ids:         []string{"-1"},
			expectError: true,
			errorMsg:    "invalid hash character",
		},
		{
			name:        "invalid pinned index",
			ids:         []string{"p0", "p-1", "pabc"},
			expectError: true,
			errorMsg:    "invalid pinned index format",
		},
		{
			name:        "invalid deleted index",
			ids:         []string{"d0", "d-1", "dabc"},
			expectError: true,
			errorMsg:    "invalid deleted index format",
		},
		{
			name:        "invalid hash characters",
			ids:         []string{"xyz", "has space", "has-dash", "has_underscore"},
			expectError: true,
			errorMsg:    "invalid hash character",
		},
		{
			name:        "mixed valid and invalid",
			ids:         []string{"1", "p1", "invalid!", "d1"},
			expectError: true,
			errorMsg:    "invalid IDs",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateIDs(tt.ids)

			if tt.expectError {
				assert.Error(t, err)
				if tt.errorMsg != "" {
					assert.Contains(t, err.Error(), tt.errorMsg)
				}
			} else {
				assert.NoError(t, err)
			}
		})
	}
}

func TestOrderPreservation(t *testing.T) {
	s := createTestStore(t)
	scratches := createTestScratches(t, s, 5)

	// Test that order is preserved exactly as specified
	ids := []string{"3", "1", "p2", "2", "d1"}
	results, err := ResolveMultipleIDs(s, false, false, "test", ids)

	require.NoError(t, err)
	require.Len(t, results, 5)

	// Check order matches input
	// Active scratches in sorted order: [2, 1, 0] (newest to oldest)
	// So index 1 -> scratches[2], index 2 -> scratches[1], index 3 -> scratches[0]
	assert.Equal(t, scratches[0].ID, results[0].ID) // "3" -> index 3 -> scratches[0]
	assert.Equal(t, scratches[2].ID, results[1].ID) // "1" -> index 1 -> scratches[2]
	assert.Equal(t, scratches[0].ID, results[2].ID) // "p2" -> second pinned -> scratches[0]
	assert.Equal(t, scratches[1].ID, results[3].ID) // "2" -> index 2 -> scratches[1]
	assert.Equal(t, scratches[4].ID, results[4].ID) // "d1" -> most recent deleted -> scratches[4]
}

func TestProjectFiltering(t *testing.T) {
	s := createTestStore(t)

	// Create scratches in different projects
	scratch1 := store.Scratch{
		ID:        "proj1",
		Project:   "project1",
		Title:     "Project 1 Scratch",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	scratch2 := store.Scratch{
		ID:        "proj2",
		Project:   "project2",
		Title:     "Project 2 Scratch",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	scratchGlobal := store.Scratch{
		ID:        "global1",
		Project:   "global",
		Title:     "Global Scratch",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}

	require.NoError(t, s.AddScratch(scratch1))
	require.NoError(t, s.AddScratch(scratch2))
	require.NoError(t, s.AddScratch(scratchGlobal))

	// Test project filtering
	results, err := ResolveMultipleIDs(s, false, false, "project1", []string{"1"})
	require.NoError(t, err)
	assert.Len(t, results, 1)
	assert.Equal(t, "proj1", results[0].ID)

	// Test global filtering
	results, err = ResolveMultipleIDs(s, false, true, "", []string{"1"})
	require.NoError(t, err)
	assert.Len(t, results, 1)
	assert.Equal(t, "global1", results[0].ID)

	// Test all flag
	results, err = ResolveMultipleIDs(s, true, false, "", []string{"1", "2", "3"})
	require.NoError(t, err)
	assert.Len(t, results, 3)
}

func TestParseIndex(t *testing.T) {
	tests := []struct {
		input   string
		want    int
		wantErr bool
	}{
		{"1", 1, false},
		{"10", 10, false},
		{"999", 999, false},
		{"", 0, true},
		{"0", 0, true},
		{"-1", 0, true},
		{"abc", 0, true},
		{"1a", 0, true},
		{"1.5", 0, true},
		{"99999999", 0, true}, // Too large
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got, err := parseIndex(tt.input)
			if tt.wantErr {
				assert.Error(t, err)
			} else {
				assert.NoError(t, err)
				assert.Equal(t, tt.want, got)
			}
		})
	}
}

func TestIsHexChar(t *testing.T) {
	tests := []struct {
		input rune
		want  bool
	}{
		{'0', true},
		{'9', true},
		{'a', true},
		{'f', true},
		{'A', true},
		{'F', true},
		{'g', false},
		{'G', false},
		{' ', false},
		{'-', false},
		{'_', false},
	}

	for _, tt := range tests {
		t.Run(string(tt.input), func(t *testing.T) {
			got := isHexChar(tt.input)
			assert.Equal(t, tt.want, got)
		})
	}
}

func BenchmarkResolveMultipleIDs(b *testing.B) {
	// Create test store manually for benchmark
	fs := filesystem.NewMemoryFileSystem()
	cfg := &config.Config{
		FileSystem: fs,
		DataPath:   "/test",
	}

	s, err := store.NewStoreWithConfig(cfg)
	if err != nil {
		b.Fatal(err)
	}

	// Create many scratches
	for i := 0; i < 1000; i++ {
		scratch := store.Scratch{
			ID:        fmt.Sprintf("hash%d", i),
			Project:   "test",
			Title:     fmt.Sprintf("Test Scratch %d", i),
			CreatedAt: time.Now(),
			UpdatedAt: time.Now(),
		}
		_ = s.AddScratch(scratch)
	}

	// Prepare test IDs
	ids := make([]string, 100)
	for i := 0; i < 100; i++ {
		ids[i] = fmt.Sprintf("%d", i+1)
	}

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_, _ = ResolveMultipleIDs(s, false, false, "test", ids)
	}
}

func TestResolveMultipleIDsConcurrent(t *testing.T) {
	// This test ensures the function is safe for concurrent use
	s := createTestStore(t)
	createTestScratches(t, s, 10)

	done := make(chan bool)
	errors := make(chan error, 10)

	// Run multiple goroutines resolving IDs concurrently
	for i := 0; i < 10; i++ {
		go func(n int) {
			ids := []string{fmt.Sprintf("%d", n%5+1), "p1", "d1"}
			_, err := ResolveMultipleIDs(s, false, false, "test", ids)
			if err != nil && !strings.Contains(err.Error(), "out of range") {
				errors <- err
			}
			done <- true
		}(i)
	}

	// Wait for all goroutines
	for i := 0; i < 10; i++ {
		<-done
	}

	// Check for unexpected errors
	select {
	case err := <-errors:
		t.Fatalf("Unexpected error in concurrent test: %v", err)
	default:
		// No errors, test passed
	}
}
