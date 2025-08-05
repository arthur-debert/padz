package commands

import (
	"github.com/arthur-debert/padz/pkg/store"
	"regexp"
)

// ScratchWithIndex wraps a Scratch with its positional index
type ScratchWithIndex struct {
	store.Scratch
	Index int `json:"index"`
}

func Search(s *store.Store, all, global bool, project, term string) ([]store.Scratch, error) {
	scratches := Ls(s, all, global, project)

	re, err := regexp.Compile(term)
	if err != nil {
		return nil, err
	}

	var filtered []store.Scratch
	for _, scratch := range scratches {
		content, err := readScratchFile(scratch.ID)
		if err != nil {
			return nil, err
		}
		if re.Match(content) {
			filtered = append(filtered, scratch)
		}
	}

	return filtered, nil
}

// SearchWithIndices performs a search and returns results with their correct positional indices
func SearchWithIndices(s *store.Store, all, global bool, project, term string) ([]ScratchWithIndex, error) {
	// Get all scratches in the correct order
	allScratches := Ls(s, all, global, project)

	// Create a map of ID to index for quick lookup
	idToIndex := make(map[string]int)
	for i, scratch := range allScratches {
		idToIndex[scratch.ID] = i + 1 // 1-based indexing
	}

	// Perform the search
	re, err := regexp.Compile(term)
	if err != nil {
		return nil, err
	}

	var results []ScratchWithIndex
	for _, scratch := range allScratches {
		content, err := readScratchFile(scratch.ID)
		if err != nil {
			return nil, err
		}
		if re.Match(content) {
			results = append(results, ScratchWithIndex{
				Scratch: scratch,
				Index:   idToIndex[scratch.ID],
			})
		}
	}

	return results, nil
}
