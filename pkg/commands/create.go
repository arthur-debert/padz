package commands

import (
	"bufio"
	"bytes"
	"crypto/sha1"
	"fmt"
	"github.com/arthur-debert/padz/pkg/clipboard"
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/editor"
	"github.com/arthur-debert/padz/pkg/store"
	"io"
	"os"
	"strings"
	"time"
)

func Create(s *store.Store, project string, content []byte) error {
	return CreateWithTitle(s, project, content, "")
}

// CreateWithStoreManager creates a scratch using the StoreManager approach
func CreateWithStoreManager(workingDir string, globalFlag bool, content []byte, title string) error {
	sm := store.NewStoreManager()

	// Get the appropriate store based on flags
	currentStore, _, err := sm.GetCurrentStore(workingDir, globalFlag)
	if err != nil {
		return fmt.Errorf("failed to get current store: %w", err)
	}

	// Extract project name from scope
	scope := ""
	if globalFlag {
		scope = "global"
	} else {
		// Get the current scope to extract project name
		_, currentScope, _ := sm.GetCurrentStore(workingDir, false)
		scope = currentScope
	}

	// Convert scope to project name (e.g., "project:padz" -> "padz")
	projectName := scope
	if strings.HasPrefix(scope, "project:") {
		projectName = strings.TrimPrefix(scope, "project:")
	}

	// Use the existing CreateWithTitle logic
	return CreateWithTitle(currentStore, projectName, content, title)
}

// CreateWithTitle creates a scratch with an optional pre-defined title
func CreateWithTitle(s *store.Store, project string, content []byte, providedTitle string) error {
	var err error
	if len(content) == 0 {
		// If we have a provided title, show it in the editor
		var initialContent []byte
		if providedTitle != "" {
			initialContent = []byte(providedTitle + "\n\n")
		}

		// Determine extension based on title
		extension := ".txt"
		if strings.HasPrefix(strings.TrimSpace(providedTitle), "#") {
			extension = ".md"
		}

		content, err = editor.OpenInEditorWithExtension(initialContent, extension)
		if err != nil {
			return err
		}
	}

	trimmedContent := trim(content)
	if len(trimmedContent) == 0 {
		return nil // Don't save empty scratches
	}

	// Use provided title if available, otherwise extract from content
	title := providedTitle
	if title == "" {
		title = getTitle(trimmedContent)
	}

	id := fmt.Sprintf("%x", sha1.Sum(trimmedContent))

	scratch := store.Scratch{
		ID:        id,
		Project:   project,
		Title:     title,
		CreatedAt: time.Now(),
	}

	// Add the scratch metadata first
	if err := s.AddScratchAtomic(scratch); err != nil {
		return err
	}

	// Save the scratch content using the store's method
	if err := s.SaveScratchContent(id, trimmedContent); err != nil {
		// If content save fails, try to remove the metadata
		// This prevents orphaned metadata entries
		return fmt.Errorf("failed to save scratch content: %w", err)
	}

	// Copy content to clipboard
	_ = clipboard.Copy(trimmedContent)

	return nil
}

// CreateWithTitleAndContent creates a scratch with a title and optional initial content
func CreateWithTitleAndContent(s *store.Store, project string, title string, initialContent []byte) error {
	var err error
	var content []byte

	// Prepare initial content for editor
	var editorContent []byte
	if title != "" && len(initialContent) > 0 {
		// Both title and initial content provided
		editorContent = []byte(title + "\n\n" + string(initialContent))
	} else if title != "" {
		// Only title provided
		editorContent = []byte(title + "\n\n")
	} else if len(initialContent) > 0 {
		// Only initial content provided
		editorContent = initialContent
	}

	// Determine extension based on title
	extension := ".txt"
	if strings.HasPrefix(strings.TrimSpace(title), "#") {
		extension = ".md"
	}

	// Open editor with prepared content
	content, err = editor.OpenInEditorWithExtension(editorContent, extension)
	if err != nil {
		return err
	}

	trimmedContent := trim(content)
	if len(trimmedContent) == 0 {
		return nil // Don't save empty scratches
	}

	// Use provided title if available, otherwise extract from content
	finalTitle := title
	if finalTitle == "" {
		finalTitle = getTitle(trimmedContent)
	}

	id := fmt.Sprintf("%x", sha1.Sum(trimmedContent))

	scratch := store.Scratch{
		ID:        id,
		Project:   project,
		Title:     finalTitle,
		CreatedAt: time.Now(),
	}

	if err := saveScratchFile(id, trimmedContent); err != nil {
		return err
	}

	if err := s.AddScratchAtomic(scratch); err != nil {
		return err
	}

	// Copy content to clipboard
	_ = clipboard.Copy(trimmedContent)

	return nil
}

func getTitle(content []byte) string {
	reader := bytes.NewReader(content)
	scanner := bufio.NewScanner(reader)
	if scanner.Scan() {
		return scanner.Text()
	}
	return "Untitled"
}

func trim(content []byte) []byte {
	return []byte(strings.Trim(string(content), "\n\t "))
}

func saveScratchFile(id string, content []byte) error {
	fs := config.GetConfig().FileSystem
	path, err := store.GetScratchFilePath(id)
	if err != nil {
		return err
	}
	return fs.WriteFile(path, content, 0644)
}

// ReadContentFromPipe checks if stdin is a pipe and reads its content
func ReadContentFromPipe() []byte {
	return ReadContentFromPipeWithReader(os.Stdin)
}

// ReadContentFromPipeWithReader reads content from a pipe using the provided reader
// This function is exported to allow for easier testing with mock readers
func ReadContentFromPipeWithReader(reader io.Reader) []byte {
	// Check if the reader is os.Stdin to perform pipe detection
	if stdin, ok := reader.(*os.File); ok && stdin == os.Stdin {
		info, err := stdin.Stat()
		if err != nil {
			return nil
		}
		if info.Mode()&os.ModeNamedPipe == 0 {
			return nil
		}
	}

	var buf bytes.Buffer
	_, _ = io.Copy(&buf, reader)
	return buf.Bytes()
}
