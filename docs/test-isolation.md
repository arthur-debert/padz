# Test Isolation Strategy

## Problem
The test suite was performing file operations (create/delete/modify) directly in the user's home directory, which is dangerous and can lead to data loss. Tests should be completely isolated from the real file system.

## Solution
We implemented a comprehensive file system abstraction layer that allows tests to run in complete isolation:

### 1. FileSystem Interface (`pkg/filesystem/filesystem.go`)
- Defines a common interface for all file operations
- Two implementations:
  - `OSFileSystem`: For production use (delegates to real OS operations)
  - `MemoryFileSystem`: For testing (stores files in memory)

### 2. Dependency Injection via Config (`pkg/config/config.go`)
- Configuration object holds the FileSystem implementation
- Production uses `OSFileSystem`
- Tests use `MemoryFileSystem`
- Configurable data path (tests use `/test/data` instead of real XDG paths)

### 3. Test Utilities (`pkg/testutil/testutil.go`)
- `SetupTestEnvironment`: Creates isolated test environment with memory filesystem
- Handles cleanup automatically
- Ensures each test starts with a clean slate

### 4. Command Test Helpers (`pkg/commands/testutil.go`)
- `SetupCommandTest`: Specialized setup for command tests
- Helper methods for writing/reading scratch files in tests
- Ensures all file operations go through the abstraction

## Benefits

1. **Complete Isolation**: Tests never touch the real file system
2. **Speed**: Memory operations are much faster than disk I/O
3. **Parallelization**: Tests can run in parallel without conflicts
4. **Deterministic**: No dependency on existing file system state
5. **Clean**: Each test starts fresh, no cleanup needed

## Usage

### In Tests
```go
func TestMyFeature(t *testing.T) {
    setup := SetupCommandTest(t)
    defer setup.Cleanup()
    
    // Use setup.Store for all operations
    err := Create(setup.Store, "project", []byte("content"))
    
    // Files are created only in memory
    // Real file system is never touched
}
```

### In Production
The production code automatically uses the real file system through the default configuration:
```go
store, err := store.NewStore() // Uses OSFileSystem by default
```

## Verification
The `isolation_test.go` file contains tests that verify:
- No files are created in the real file system
- Each test gets a fresh environment
- Memory file system correctly isolates all operations

## Future Improvements
- Could add a `TempDirFileSystem` for integration tests that need real file operations but still want isolation
- Could add file system operation logging for debugging
- Could implement quota limits on the memory file system
