package store

import (
	"path/filepath"
	"testing"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/testutil"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestStoreManager_GetGlobalStore(t *testing.T) {
	_, cleanup := testutil.SetupMultiScopeTestEnvironment(t, "global")
	defer cleanup()

	sm := NewStoreManager()

	t.Run("get global store", func(t *testing.T) {
		store, err := sm.GetGlobalStore()
		require.NoError(t, err)
		assert.NotNil(t, store)

		// Verify it's cached
		store2, err := sm.GetGlobalStore()
		require.NoError(t, err)
		assert.Same(t, store, store2)
	})
}

func TestStoreManager_GetProjectStore(t *testing.T) {
	env, cleanup := testutil.SetupMultiScopeTestEnvironment(t, "myproject", "otherproject")
	defer cleanup()

	sm := NewStoreManager()

	t.Run("get project store", func(t *testing.T) {
		projectDir := env.GetProjectDir("myproject")
		store, err := sm.GetProjectStore("myproject", projectDir)
		require.NoError(t, err)
		assert.NotNil(t, store)

		// Verify it's cached
		store2, err := sm.GetProjectStore("myproject", projectDir)
		require.NoError(t, err)
		assert.Same(t, store, store2)
	})

	t.Run("reject global scope", func(t *testing.T) {
		_, err := sm.GetProjectStore("global", "")
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "use GetGlobalStore() for global scope")
	})

	t.Run("different projects get different stores", func(t *testing.T) {
		projectDir1 := env.GetProjectDir("myproject")
		store1, err := sm.GetProjectStore("myproject", projectDir1)
		require.NoError(t, err)

		projectDir2 := env.GetProjectDir("otherproject")
		store2, err := sm.GetProjectStore("otherproject", projectDir2)
		require.NoError(t, err)

		assert.NotSame(t, store1, store2)
	})
}

func TestStoreManager_GetStore_Deprecated(t *testing.T) {
	env, cleanup := testutil.SetupMultiScopeTestEnvironment(t, "global", "myproject")
	defer cleanup()

	sm := NewStoreManager()

	t.Run("legacy GetStore still works for global", func(t *testing.T) {
		store, err := sm.GetStore("global", "")
		require.NoError(t, err)
		assert.NotNil(t, store)
	})

	t.Run("legacy GetStore still works for project", func(t *testing.T) {
		projectDir := env.GetProjectDir("myproject")
		store, err := sm.GetStore("myproject", projectDir)
		require.NoError(t, err)
		assert.NotNil(t, store)
	})

	t.Run("global and project stores are different", func(t *testing.T) {
		globalStore, err := sm.GetStore("global", "")
		require.NoError(t, err)

		projectDir := env.GetProjectDir("myproject")
		projectStore, err := sm.GetStore("myproject", projectDir)
		require.NoError(t, err)

		assert.NotSame(t, globalStore, projectStore)
	})
}

func TestStoreManager_GetCurrentStore(t *testing.T) {
	env, cleanup := testutil.SetupMultiScopeTestEnvironment(t, "global", "myproject")
	defer cleanup()

	sm := NewStoreManager()

	t.Run("global flag forces global store", func(t *testing.T) {
		projectDir := env.GetProjectDir("myproject")
		store, scope, err := sm.GetCurrentStore(projectDir, true)
		require.NoError(t, err)
		assert.Equal(t, "global", scope)
		assert.NotNil(t, store)
	})

	t.Run("project directory returns project store", func(t *testing.T) {
		projectDir := env.GetProjectDir("myproject")
		store, scope, err := sm.GetCurrentStore(projectDir, false)
		require.NoError(t, err)
		assert.Equal(t, "myproject", scope)
		assert.NotNil(t, store)
	})

	t.Run("non-git directory returns global", func(t *testing.T) {
		// Create a directory without .git
		nonGitDir := "/test/nogit"
		err := env.MemFS.MkdirAll(nonGitDir, 0755)
		require.NoError(t, err)

		store, scope, err := sm.GetCurrentStore(nonGitDir, false)
		require.NoError(t, err)
		assert.Equal(t, "global", scope)
		assert.NotNil(t, store)
	})
}

func TestStoreManager_getStorePath(t *testing.T) {
	env, cleanup := testutil.SetupMultiScopeTestEnvironment(t, "myproject")
	defer cleanup()

	sm := NewStoreManager()

	t.Run("global scope path with configured DataPath", func(t *testing.T) {
		path, err := sm.getStorePath("global", "")
		require.NoError(t, err)
		expected := filepath.Join(env.BaseDir, "data", "global")
		assert.Equal(t, expected, path)
	})

	t.Run("global scope path without configured DataPath", func(t *testing.T) {
		// Save original config
		originalConfig := config.GetConfig()
		defer config.SetConfig(originalConfig)

		// Set config without DataPath to test XDG path behavior
		prodConfig := &config.Config{
			FileSystem: env.Config.FileSystem,
			DataPath:   "", // Empty to trigger XDG path
		}
		config.SetConfig(prodConfig)

		path, err := sm.getStorePath("global", "")
		require.NoError(t, err)
		// Path should be from XDG but still a valid path
		assert.NotEmpty(t, path)
		assert.Contains(t, path, "global")
	})

	t.Run("project scope path", func(t *testing.T) {
		projectDir := env.GetProjectDir("myproject")
		path, err := sm.getStorePath("myproject", projectDir)
		require.NoError(t, err)
		expected := filepath.Join(projectDir, ".padz", "scratch", "myproject")
		assert.Equal(t, expected, path)
	})

	t.Run("project scope path uses filesystem abstraction", func(t *testing.T) {
		projectDir := env.GetProjectDir("myproject")
		path, err := sm.getStorePath("myproject", projectDir)
		require.NoError(t, err)

		// Verify the path uses the filesystem's Join method by checking separator consistency
		expectedParts := []string{projectDir, ".padz", "scratch", "myproject"}
		expected := env.Config.FileSystem.Join(expectedParts...)
		assert.Equal(t, expected, path)
	})
}

func TestStoreManager_findGitRoot(t *testing.T) {
	env, cleanup := testutil.SetupMultiScopeTestEnvironment(t, "myproject")
	defer cleanup()

	sm := NewStoreManager()

	t.Run("finds git root from project directory", func(t *testing.T) {
		projectDir := env.GetProjectDir("myproject")
		gitRoot, err := sm.findGitRoot(projectDir)
		require.NoError(t, err)
		assert.Equal(t, projectDir, gitRoot)
	})

	t.Run("finds git root from subdirectory", func(t *testing.T) {
		projectDir := env.GetProjectDir("myproject")
		subDir := filepath.Join(projectDir, "src", "deep", "nested")
		err := env.MemFS.MkdirAll(subDir, 0755)
		require.NoError(t, err)

		gitRoot, err := sm.findGitRoot(subDir)
		require.NoError(t, err)
		assert.Equal(t, projectDir, gitRoot)
	})

	t.Run("fails when no git root found", func(t *testing.T) {
		nonGitDir := "/test/nogit"
		err := env.MemFS.MkdirAll(nonGitDir, 0755)
		require.NoError(t, err)

		_, err = sm.findGitRoot(nonGitDir)
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "no git repository found")
	})
}

func TestStoreManager_Integration(t *testing.T) {
	env, cleanup := testutil.SetupMultiScopeTestEnvironment(t, "proj1", "proj2")
	defer cleanup()

	sm := NewStoreManager()

	// Get stores for different projects
	proj1Dir := env.GetProjectDir("proj1")
	proj1Store, err := sm.GetStore("proj1", proj1Dir)
	require.NoError(t, err)

	proj2Dir := env.GetProjectDir("proj2")
	proj2Store, err := sm.GetStore("proj2", proj2Dir)
	require.NoError(t, err)

	globalStore, err := sm.GetStore("global", "")
	require.NoError(t, err)

	// Add scratches to different stores
	proj1Scratch := Scratch{
		ID:      "proj1-scratch",
		Project: "proj1",
		Title:   "Project 1 Scratch",
	}
	err = proj1Store.AddScratch(proj1Scratch)
	require.NoError(t, err)

	proj2Scratch := Scratch{
		ID:      "proj2-scratch",
		Project: "proj2",
		Title:   "Project 2 Scratch",
	}
	err = proj2Store.AddScratch(proj2Scratch)
	require.NoError(t, err)

	globalScratch := Scratch{
		ID:      "global-scratch",
		Project: "global",
		Title:   "Global Scratch",
	}
	err = globalStore.AddScratch(globalScratch)
	require.NoError(t, err)

	// Verify isolation
	assert.Len(t, proj1Store.GetScratches(), 1)
	assert.Len(t, proj2Store.GetScratches(), 1)
	assert.Len(t, globalStore.GetScratches(), 1)

	// Verify correct scratches in each store
	proj1Scratches := proj1Store.GetScratches()
	assert.Equal(t, "Project 1 Scratch", proj1Scratches[0].Title)

	proj2Scratches := proj2Store.GetScratches()
	assert.Equal(t, "Project 2 Scratch", proj2Scratches[0].Title)

	globalScratches := globalStore.GetScratches()
	assert.Equal(t, "Global Scratch", globalScratches[0].Title)
}
