package commands

import (
	"bufio"
	"bytes"
	"github.com/arthur-debert/padz/pkg/store"
	"strings"
)

func Peek(s *store.Store, all, global bool, project string, indexStr string, lines int) (string, error) {
	content, err := View(s, all, global, project, indexStr)
	if err != nil {
		return "", err
	}

	scanner := bufio.NewScanner(strings.NewReader(content))
	var contentLines []string
	for scanner.Scan() {
		contentLines = append(contentLines, scanner.Text())
	}

	if len(contentLines) <= 2*lines {
		return content, nil
	}

	var result bytes.Buffer
	for i := 0; i < lines; i++ {
		result.WriteString(contentLines[i])
		result.WriteString("\n")
	}
	result.WriteString("...\n")
	for i := len(contentLines) - lines; i < len(contentLines); i++ {
		result.WriteString(contentLines[i])
		result.WriteString("\n")
	}

	return result.String(), nil
}

// PeekWithStoreManager shows a preview of scratch content using StoreManager
func PeekWithStoreManager(workingDir string, globalFlag bool, indexStr string, lines int) (string, error) {
	content, err := ViewWithStoreManager(workingDir, globalFlag, indexStr)
	if err != nil {
		return "", err
	}

	scanner := bufio.NewScanner(strings.NewReader(content))
	var contentLines []string
	for scanner.Scan() {
		contentLines = append(contentLines, scanner.Text())
	}

	if len(contentLines) <= 2*lines {
		return content, nil
	}

	var result bytes.Buffer
	for i := 0; i < lines; i++ {
		result.WriteString(contentLines[i])
		result.WriteString("\n")
	}
	result.WriteString("...\n")
	for i := len(contentLines) - lines; i < len(contentLines); i++ {
		result.WriteString(contentLines[i])
		result.WriteString("\n")
	}

	return result.String(), nil
}
