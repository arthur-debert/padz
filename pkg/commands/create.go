package commands

import (
	"bufio"
	"bytes"
	"crypto/sha1"
	"fmt"
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

// CreateWithTitle creates a scratch with an optional pre-defined title
func CreateWithTitle(s *store.Store, project string, content []byte, providedTitle string) error {
	var err error
	if len(content) == 0 {
		// If we have a provided title, show it in the editor
		var initialContent []byte
		if providedTitle != "" {
			initialContent = []byte(providedTitle + "\n\n")
		}
		content, err = editor.OpenInEditor(initialContent)
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

	if err := saveScratchFile(id, trimmedContent); err != nil {
		return err
	}

	return s.AddScratch(scratch)
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
