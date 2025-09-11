package store

import (
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/testutil"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestMergedStore_GetAllScratches(t *testing.T) {
	env, cleanup := testutil.SetupMultiScopeTestEnvironment(t, "proj1", "proj2")
	defer cleanup()

	sm := NewStoreManager()

	// Setup stores with test data
	setupTestScratches := func() {
		// Project 1 scratches
		proj1Dir := env.GetProjectDir("proj1")
		proj1Store, err := sm.GetStore("proj1", proj1Dir)
		require.NoError(t, err)

		proj1Scratch1 := Scratch{
			ID:        "proj1-1",
			Project:   "proj1",
			Title:     "Project 1 First",
			CreatedAt: time.Now().Add(-2 * time.Hour),
		}
		proj1Scratch2 := Scratch{
			ID:        "proj1-2",
			Project:   "proj1",
			Title:     "Project 1 Second",
			CreatedAt: time.Now().Add(-1 * time.Hour),
		}
		require.NoError(t, proj1Store.AddScratch(proj1Scratch1))
		require.NoError(t, proj1Store.AddScratch(proj1Scratch2))

		// Project 2 scratches
		proj2Dir := env.GetProjectDir("proj2")
		proj2Store, err := sm.GetStore("proj2", proj2Dir)
		require.NoError(t, err)

		proj2Scratch := Scratch{
			ID:        "proj2-1",
			Project:   "proj2",
			Title:     "Project 2 First",
			CreatedAt: time.Now().Add(-30 * time.Minute),
		}
		require.NoError(t, proj2Store.AddScratch(proj2Scratch))

		// Global scratches
		globalStore, err := sm.GetStore("global", "")
		require.NoError(t, err)

		globalScratch := Scratch{
			ID:        "global-1",
			Project:   "global",
			Title:     "Global First",
			CreatedAt: time.Now(),
		}
		require.NoError(t, globalStore.AddScratch(globalScratch))
	}

	setupTestScratches()

	// Create merged store
	workingDirs := map[string]string{
		"proj1":  env.GetProjectDir("proj1"),
		"proj2":  env.GetProjectDir("proj2"),
		"global": "",
	}
	mergedStore, err := NewMergedStore(sm, []string{"proj1", "proj2", "global"}, workingDirs)
	require.NoError(t, err)

	t.Run("get all scratches sorted by most recent", func(t *testing.T) {
		scratches := mergedStore.GetAllScratches(false)
		require.Len(t, scratches, 4)

		// Should be sorted by most recent first
		assert.Equal(t, "Global First", scratches[0].Title)
		assert.Equal(t, "global:1", scratches[0].ScopedID)
		assert.Equal(t, "global", scratches[0].Scope)

		assert.Equal(t, "Project 2 First", scratches[1].Title)
		assert.Equal(t, "proj2:1", scratches[1].ScopedID)
		assert.Equal(t, "proj2", scratches[1].Scope)

		assert.Equal(t, "Project 1 Second", scratches[2].Title)
		assert.Equal(t, "proj1:1", scratches[2].ScopedID) // proj1 sorted internally by newest first
		assert.Equal(t, "proj1", scratches[2].Scope)

		assert.Equal(t, "Project 1 First", scratches[3].Title)
		assert.Equal(t, "proj1:2", scratches[3].ScopedID)
		assert.Equal(t, "proj1", scratches[3].Scope)
	})

	t.Run("scoped IDs are correctly formatted", func(t *testing.T) {
		scratches := mergedStore.GetAllScratches(false)

		scopedIDs := make([]string, len(scratches))
		for i, scratch := range scratches {
			scopedIDs[i] = scratch.ScopedID
		}

		expected := []string{"global:1", "proj2:1", "proj1:1", "proj1:2"}
		assert.Equal(t, expected, scopedIDs)
	})
}

func TestMergedStore_GetScopedScratch(t *testing.T) {
	env, cleanup := testutil.SetupMultiScopeTestEnvironment(t, "myproject")
	defer cleanup()

	sm := NewStoreManager()

	// Setup test data
	projectDir := env.GetProjectDir("myproject")
	projectStore, err := sm.GetStore("myproject", projectDir)
	require.NoError(t, err)

	testScratch := Scratch{
		ID:      "test-scratch",
		Project: "myproject",
		Title:   "Test Scratch",
	}
	require.NoError(t, projectStore.AddScratch(testScratch))

	// Create merged store
	workingDirs := map[string]string{
		"myproject": projectDir,
	}
	mergedStore, err := NewMergedStore(sm, []string{"myproject"}, workingDirs)
	require.NoError(t, err)

	t.Run("valid scoped ID", func(t *testing.T) {
		scopedScratch, err := mergedStore.GetScopedScratch("myproject:1")
		require.NoError(t, err)
		assert.Equal(t, "Test Scratch", scopedScratch.Title)
		assert.Equal(t, "myproject", scopedScratch.Scope)
		assert.Equal(t, "myproject:1", scopedScratch.ScopedID)
		assert.Equal(t, 1, scopedScratch.Index)
	})

	t.Run("invalid scoped ID format", func(t *testing.T) {
		_, err := mergedStore.GetScopedScratch("invalid")
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "invalid scoped ID format")
	})

	t.Run("non-existent scope", func(t *testing.T) {
		_, err := mergedStore.GetScopedScratch("nonexistent:1")
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "scope nonexistent not found")
	})

	t.Run("invalid index in scope", func(t *testing.T) {
		_, err := mergedStore.GetScopedScratch("myproject:999")
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "index 999 out of range in scope myproject")
	})
}

func TestMergedStore_GetAvailableScopes(t *testing.T) {
	env, cleanup := testutil.SetupMultiScopeTestEnvironment(t, "proj1", "proj2")
	defer cleanup()

	sm := NewStoreManager()

	workingDirs := map[string]string{
		"proj1":  env.GetProjectDir("proj1"),
		"proj2":  env.GetProjectDir("proj2"),
		"global": "",
	}
	mergedStore, err := NewMergedStore(sm, []string{"proj1", "proj2", "global"}, workingDirs)
	require.NoError(t, err)

	scopes := mergedStore.GetAvailableScopes()
	expected := []string{"global", "proj1", "proj2"}
	assert.Equal(t, expected, scopes)
}

func TestValidateNoScopedIDs(t *testing.T) {
	tests := []struct {
		name        string
		ids         []string
		expectError bool
		errorMsg    string
	}{
		{
			name:        "all plain IDs",
			ids:         []string{"1", "2", "3"},
			expectError: false,
		},
		{
			name:        "single scoped ID",
			ids:         []string{"1", "global:2", "3"},
			expectError: true,
			errorMsg:    "scoped IDs not allowed",
		},
		{
			name:        "multiple scoped IDs",
			ids:         []string{"proj1:1", "proj2:2"},
			expectError: true,
			errorMsg:    "proj1:1, proj2:2",
		},
		{
			name:        "empty slice",
			ids:         []string{},
			expectError: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := ValidateNoScopedIDs(tt.ids)
			if tt.expectError {
				assert.Error(t, err)
				assert.Contains(t, err.Error(), tt.errorMsg)
			} else {
				assert.NoError(t, err)
			}
		})
	}
}

func TestExtractScopesFromIDs(t *testing.T) {
	tests := []struct {
		name     string
		ids      []string
		expected []string
	}{
		{
			name:     "no scoped IDs",
			ids:      []string{"1", "2", "3"},
			expected: []string{},
		},
		{
			name:     "single scope",
			ids:      []string{"global:1", "global:2"},
			expected: []string{"global"},
		},
		{
			name:     "multiple scopes",
			ids:      []string{"proj1:1", "global:2", "proj2:3"},
			expected: []string{"global", "proj1", "proj2"},
		},
		{
			name:     "mixed scoped and unscoped",
			ids:      []string{"1", "proj1:2", "3", "global:4"},
			expected: []string{"global", "proj1"},
		},
		{
			name:     "duplicate scopes",
			ids:      []string{"proj1:1", "proj1:2", "proj1:3"},
			expected: []string{"proj1"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := ExtractScopesFromIDs(tt.ids)
			assert.Equal(t, tt.expected, result)
		})
	}
}

func TestMergedStore_Integration(t *testing.T) {
	env, cleanup := testutil.SetupMultiScopeTestEnvironment(t, "webapp", "mobile")
	defer cleanup()

	sm := NewStoreManager()

	// Create scratches in different scopes
	now := time.Now()

	// Webapp project
	webappDir := env.GetProjectDir("webapp")
	webappStore, err := sm.GetStore("webapp", webappDir)
	require.NoError(t, err)

	webappScratch1 := Scratch{
		ID:        "webapp-1",
		Project:   "webapp",
		Title:     "Fix login bug",
		CreatedAt: now.Add(-3 * time.Hour),
		UpdatedAt: now.Add(-3 * time.Hour),
	}
	webappScratch2 := Scratch{
		ID:        "webapp-2",
		Project:   "webapp",
		Title:     "Add dark mode",
		CreatedAt: now.Add(-1 * time.Hour),
		UpdatedAt: now.Add(-1 * time.Hour),
	}
	require.NoError(t, webappStore.AddScratch(webappScratch1))
	require.NoError(t, webappStore.AddScratch(webappScratch2))

	// Mobile project
	mobileDir := env.GetProjectDir("mobile")
	mobileStore, err := sm.GetStore("mobile", mobileDir)
	require.NoError(t, err)

	mobileScratch := Scratch{
		ID:        "mobile-1",
		Project:   "mobile",
		Title:     "Push notifications",
		CreatedAt: now.Add(-2 * time.Hour),
		UpdatedAt: now.Add(-2 * time.Hour),
	}
	require.NoError(t, mobileStore.AddScratch(mobileScratch))

	// Global scope
	globalStore, err := sm.GetStore("global", "")
	require.NoError(t, err)

	globalScratch := Scratch{
		ID:        "global-1",
		Project:   "global",
		Title:     "Weekend plans",
		CreatedAt: now,
		UpdatedAt: now,
	}
	require.NoError(t, globalStore.AddScratch(globalScratch))

	// Create merged store
	workingDirs := map[string]string{
		"webapp": webappDir,
		"mobile": mobileDir,
		"global": "",
	}
	mergedStore, err := NewMergedStore(sm, []string{"webapp", "mobile", "global"}, workingDirs)
	require.NoError(t, err)

	// Test cross-scope listing
	allScratches := mergedStore.GetAllScratches(false)
	require.Len(t, allScratches, 4)

	// Verify ordering (most recent first)
	titles := make([]string, len(allScratches))
	scopedIDs := make([]string, len(allScratches))
	for i, scratch := range allScratches {
		titles[i] = scratch.Title
		scopedIDs[i] = scratch.ScopedID
	}

	// Expected order by creation time (most recent first):
	// 1. "Weekend plans" (now)
	// 2. "Add dark mode" (now - 1 hour)
	// 3. "Push notifications" (now - 2 hours)
	// 4. "Fix login bug" (now - 3 hours)
	expectedTitles := []string{"Weekend plans", "Add dark mode", "Push notifications", "Fix login bug"}
	expectedScopedIDs := []string{"global:1", "webapp:1", "mobile:1", "webapp:2"}

	assert.Equal(t, expectedTitles, titles)
	assert.Equal(t, expectedScopedIDs, scopedIDs)

	// Test scoped ID resolution
	scopedScratch, err := mergedStore.GetScopedScratch("webapp:1")
	require.NoError(t, err)
	assert.Equal(t, "Add dark mode", scopedScratch.Title)
	assert.Equal(t, "webapp", scopedScratch.Scope)
}
