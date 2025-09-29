package commands

import (
	"bufio"
	"bytes"
	"crypto/sha1"
	"fmt"
	"github.com/arthur-debert/padz/pkg/clipboard"
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

	scratch := store.Scratch{
		Project:   project,
		Title:     title,
		Content:   string(trimmedContent), // Store content directly in the scratch
		CreatedAt: time.Now(),
	}

	// Calculate size and checksum for metadata
	scratch.Size = int64(len(trimmedContent))
	scratch.Checksum = fmt.Sprintf("%x", sha1.Sum(trimmedContent))

	if err := s.AddScratchAtomic(scratch); err != nil {
		return err
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

	scratch := store.Scratch{
		Project:   project,
		Title:     finalTitle,
		Content:   string(trimmedContent), // Store content directly in the scratch
		CreatedAt: time.Now(),
	}

	// Calculate size and checksum for metadata
	scratch.Size = int64(len(trimmedContent))
	scratch.Checksum = fmt.Sprintf("%x", sha1.Sum(trimmedContent))

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
