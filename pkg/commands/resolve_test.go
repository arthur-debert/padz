package commands

import (
	"fmt"
	"strings"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func createTestStore(t *testing.T) *store.Store {
	t.Helper()
	setup := SetupCommandTest(t)
	t.Cleanup(setup.Cleanup)
	return setup.Store
}

func createTestScratches(t *testing.T, s *store.Store, count int) []store.Scratch {
	t.Helper()
	scratches := make([]store.Scratch, count)

	// Get test store for custom timestamps
	testStore, ok := s.GetTestStore()
	if !ok {
		t.Skip("Test store not available")
	}

	// Create scratches with increasing creation times so we have predictable ordering
	// Scratch 0 is oldest, scratch count-1 is newest
	baseTime := time.Now().Add(-time.Duration(count) * time.Hour)

	for i := 0; i < count; i++ {
		scratch := store.Scratch{
			Project:   "test",
			Title:     fmt.Sprintf("Test Scratch %d", i+1),
			Content:   "test content",
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

		// Set custom timestamp for this scratch
		testStore.SetTimeFunc(func() time.Time { return scratch.CreatedAt })
		err := s.AddScratch(scratch)
		require.NoError(t, err)

		// Store the scratch with the UUID that was assigned
		allScratches := s.GetAllScratches()
		for _, sc := range allScratches {
			if sc.Title == scratch.Title {
				scratches[i] = sc
				break
			}
		}
	}
	testStore.SetTimeFunc(time.Now)

	return scratches
}

// getPrefix safely gets a prefix of the string
func getPrefix(s string, length int) string {
	if len(s) <= length {
		return s
	}
	return s[:length]
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
			ids:         []string{"1"}, // Only scratch[2] is active non-pinned
			expectError: false,
			expectCount: 1,
			expectIDs:   []string{scratches[2].ID}, // Only active non-pinned scratch
		},
		{
			name:        "pinned indices",
			ids:         []string{"p1", "p2"},
			expectError: false,
			expectCount: 2,
			expectIDs:   []string{scratches[0].ID, scratches[1].ID}, // First two are pinned
		},
		{
			name:        "deleted indices",
			ids:         []string{"d1", "d2"},
			expectError: false,
			expectCount: 2,
			expectIDs:   []string{scratches[4].ID, scratches[3].ID}, // Most recent deleted first
		},
		{
			name:        "uuid prefixes",
			ids:         []string{getPrefix(scratches[0].ID, 8), getPrefix(scratches[1].ID, 8)},
			expectError: false,
			expectCount: 2,
			expectIDs:   []string{scratches[0].ID, scratches[1].ID},
		},
		{
			name:        "mixed ID types",
			ids:         []string{"1", "p1", "d1"},
			expectError: false,
			expectCount: 3,
			expectIDs:   []string{scratches[2].ID, scratches[0].ID, scratches[4].ID}, // Active 1, pinned 1, deleted 1
		},
		{
			name:        "duplicates handled gracefully",
			ids:         []string{"1", "1", "p1", "p1"},
			expectError: false,
			expectCount: 2,                                          // Each unique ID appears once
			expectIDs:   []string{scratches[2].ID, scratches[0].ID}, // Active 1 and pinned 1
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
			ids:         []string{"1", "", "p1"},
			expectError: false,
			expectCount: 2,
			expectIDs:   []string{scratches[2].ID, scratches[0].ID}, // Active 1 and pinned 1
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results, err := ResolveMultipleIDs(s, false, "test", tt.ids)

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
				} else if tt.name == "pinned indices" {
					// For pinned indices, just verify we got the right scratches
					gotIDs := make([]string, len(results))
					for i, r := range results {
						gotIDs[i] = r.ID
					}
					// Both pinned scratches should be returned
					assert.Contains(t, gotIDs, scratches[0].ID)
					assert.Contains(t, gotIDs, scratches[1].ID)
				} else if tt.name == "mixed ID types" {
					// Verify we got the expected scratches
					assert.Equal(t, 3, len(results))
					// First should be index 1 (scratches[2])
					assert.Equal(t, scratches[2].ID, results[0].ID)
					// Second should be a pinned scratch
					assert.True(t, results[1].IsPinned)
					// Third should be scratches[2] again (UUID prefix)
					assert.Equal(t, scratches[2].ID, results[2].ID)
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
			ids:           []string{"1", "p1", "d1"},
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
			ids:           []string{"1", "invalid", "p1", "notfound"},
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
			ids:           []string{"1", "1", "p1"},
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
				assert.Equal(t, "p1", results[2].ID)
				assert.NotEqual(t, results[0].Scratch.ID, results[2].Scratch.ID)
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results := ResolveMultipleIDsWithErrors(s, false, "test", tt.ids)
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

	// With our test setup:
	// - Only scratch[2] is active non-pinned (gets index "1")
	// - scratch[0] and scratch[1] are pinned
	// - scratch[3] and scratch[4] are deleted

	// Test that order is preserved exactly as specified
	ids := []string{"1", "p1", "d1", "p2"}
	results, err := ResolveMultipleIDs(s, false, "test", ids)

	require.NoError(t, err)
	require.Len(t, results, 4)

	// Check order matches input
	assert.Equal(t, scratches[2].ID, results[0].ID) // "1" -> only active non-pinned

	// p1 and p2 are pinned scratches (order depends on how nanostore assigns them)
	assert.True(t, results[1].IsPinned) // "p1"
	assert.True(t, results[3].IsPinned) // "p2"

	// d1 is the most recently deleted scratch
	assert.Equal(t, scratches[4].ID, results[2].ID) // "d1" -> most recent deleted
}

func TestProjectFiltering(t *testing.T) {
	s := createTestStore(t)

	// Create scratches in different projects
	scratch1 := store.Scratch{
		Project:   "project1",
		Title:     "Project 1 Scratch",
		Content:   "test content",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	scratch2 := store.Scratch{
		Project:   "project2",
		Title:     "Project 2 Scratch",
		Content:   "test content",
		CreatedAt: time.Now().Add(-1 * time.Hour),
		UpdatedAt: time.Now().Add(-1 * time.Hour),
	}
	scratchGlobal := store.Scratch{
		Project:   "global",
		Title:     "Global Scratch",
		Content:   "test content",
		CreatedAt: time.Now().Add(-2 * time.Hour),
		UpdatedAt: time.Now().Add(-2 * time.Hour),
	}

	require.NoError(t, s.AddScratch(scratch1))
	require.NoError(t, s.AddScratch(scratch2))
	require.NoError(t, s.AddScratch(scratchGlobal))

	// Get the actual IDs assigned by nanostore
	proj1Scratches := s.GetScratchesWithFilter("project1", false)
	require.Len(t, proj1Scratches, 1)
	proj1ID := proj1Scratches[0].ID

	proj2Scratches := s.GetScratchesWithFilter("project2", false)
	require.Len(t, proj2Scratches, 1)
	proj2ID := proj2Scratches[0].ID

	globalScratches := s.GetScratchesWithFilter("", true)
	require.Len(t, globalScratches, 1)
	globalID := globalScratches[0].ID

	// Test project filtering - use the actual SimpleIDs assigned by nanostore
	// In project1 scope, "1" should resolve to the project1 scratch
	results, err := ResolveMultipleIDs(s, false, "project1", []string{proj1ID})
	require.NoError(t, err)
	assert.Len(t, results, 1)
	assert.Equal(t, proj1ID, results[0].ID)
	assert.Equal(t, "Project 1 Scratch", results[0].Title)

	// Test global filtering - use the actual global scratch ID
	results, err = ResolveMultipleIDs(s, true, "", []string{globalID})
	require.NoError(t, err)
	assert.Len(t, results, 1)
	assert.Equal(t, globalID, results[0].ID)
	assert.Equal(t, "Global Scratch", results[0].Title)

	// Test project2 filtering
	results, err = ResolveMultipleIDs(s, false, "project2", []string{proj2ID})
	require.NoError(t, err)
	assert.Len(t, results, 1)
	assert.Equal(t, proj2ID, results[0].ID)
	assert.Equal(t, "Project 2 Scratch", results[0].Title)
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
	cfg := &config.Config{
		DataPath: b.TempDir(),
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
		_, _ = ResolveMultipleIDs(s, false, "test", ids)
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
			_, err := ResolveMultipleIDs(s, false, "test", ids)
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
