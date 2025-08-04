package commands

import (
	"bufio"
	"bytes"
	"crypto/sha1"
	"fmt"
	"io"
	"os"
	"github.com/arthur-debert/padz/pkg/editor"
	"github.com/arthur-debert/padz/pkg/store"
	"strings"
	"time"
)

func Create(s *store.Store, project string, content []byte) error {
	var err error
	if len(content) == 0 {
		content, err = editor.OpenInEditor(nil)
		if err != nil {
			return err
		}
	}

	trimmedContent := trim(content)
	if len(trimmedContent) == 0 {
		return nil // Don't save empty scratches
	}

	title := getTitle(trimmedContent)
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
	path, err := store.GetScratchFilePath(id)
	if err != nil {
		return err
	}
	return os.WriteFile(path, content, 0644)
}

func ReadContentFromPipe() []byte {
	info, err := os.Stdin.Stat()
	if err != nil {
		return nil
	}
	if info.Mode()&os.ModeNamedPipe == 0 {
		return nil
	}
	var buf bytes.Buffer
	io.Copy(&buf, os.Stdin)
	return buf.Bytes()
}
