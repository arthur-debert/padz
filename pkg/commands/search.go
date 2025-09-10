package commands

import (
	"github.com/arthur-debert/padz/pkg/store"
	"regexp"
	"sort"
	"strings"
)

// ScratchWithIndex wraps a Scratch with its positional index
type ScratchWithIndex struct {
	store.Scratch
	Index int `json:"index"`
}

// searchResult is an internal struct to hold search ranking information
type searchResult struct {
	scratch     store.Scratch
	index       int
	titleMatch  bool
	exactMatch  bool
	matchLength int
	originalPos int
}

func Search(s *store.Store, all, global bool, project, term string) ([]store.Scratch, error) {
	return SearchWithMode(s, all, global, project, term, ListModeActive)
}

// SearchWithMode performs a search with delete filtering options
func SearchWithMode(s *store.Store, all, global bool, project, term string, mode ListMode) ([]store.Scratch, error) {
	scratches := LsWithMode(s, all, global, project, mode)

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
	return SearchWithIndicesMode(s, all, global, project, term, ListModeActive)
}

// SearchWithIndicesMode performs a search with mode and returns results with their correct positional indices
func SearchWithIndicesMode(s *store.Store, all, global bool, project, term string, mode ListMode) ([]ScratchWithIndex, error) {
	// Get all scratches in the correct order
	allScratches := LsWithMode(s, all, global, project, mode)

	// Create a map of ID to index for quick lookup
	idToIndex := make(map[string]int)
	for i, scratch := range allScratches {
		idToIndex[scratch.ID] = i + 1 // 1-based indexing
	}

	// Try to compile as user regex first
	userRe, userReErr := regexp.Compile(term)
	if userReErr != nil {
		// If it's an invalid regex, return the error
		return nil, userReErr
	}

	// Also compile case-insensitive version for literal matching
	re, err := regexp.Compile("(?i)" + regexp.QuoteMeta(term))
	if err != nil {
		return nil, err
	}

	var searchResults []searchResult

	for i, scratch := range allScratches {
		content, err := readScratchFile(scratch.ID)
		if err != nil {
			return nil, err
		}

		contentStr := string(content)
		titleLower := strings.ToLower(scratch.Title)
		termLower := strings.ToLower(term)

		// Check for matches
		titleMatch := false
		bodyMatch := false
		exactMatch := false
		matchLength := 0

		// Check title matches
		if strings.Contains(titleLower, termLower) {
			titleMatch = true
			// Check if it's an exact match
			if titleLower == termLower {
				exactMatch = true
			}
			// Find the match length
			if loc := re.FindStringIndex(scratch.Title); loc != nil {
				matchLength = loc[1] - loc[0]
			}
		}

		// Check body matches (only if no title match)
		if !titleMatch {
			if re.MatchString(contentStr) || userRe.MatchString(contentStr) {
				bodyMatch = true
				// Find longest match in content
				if locs := re.FindAllStringIndex(contentStr, -1); len(locs) > 0 {
					for _, loc := range locs {
						length := loc[1] - loc[0]
						if length > matchLength {
							matchLength = length
						}
					}
				}
				// Also check with user regex
				if locs := userRe.FindAllStringIndex(contentStr, -1); len(locs) > 0 {
					for _, loc := range locs {
						length := loc[1] - loc[0]
						if length > matchLength {
							matchLength = length
						}
					}
				}
			}
		}

		// Add to results if matched
		if titleMatch || bodyMatch {
			searchResults = append(searchResults, searchResult{
				scratch:     scratch,
				index:       idToIndex[scratch.ID],
				titleMatch:  titleMatch,
				exactMatch:  exactMatch,
				matchLength: matchLength,
				originalPos: i,
			})
		}
	}

	// Sort results according to ranking rules
	sort.Slice(searchResults, func(i, j int) bool {
		// 1. Exact matches first
		if searchResults[i].exactMatch != searchResults[j].exactMatch {
			return searchResults[i].exactMatch
		}

		// 2. Title matches before body matches
		if searchResults[i].titleMatch != searchResults[j].titleMatch {
			return searchResults[i].titleMatch
		}

		// 3. Longer matches first
		if searchResults[i].matchLength != searchResults[j].matchLength {
			return searchResults[i].matchLength > searchResults[j].matchLength
		}

		// 4. Original order
		return searchResults[i].originalPos < searchResults[j].originalPos
	})

	// Convert to final result format
	var results []ScratchWithIndex
	for _, sr := range searchResults {
		results = append(results, ScratchWithIndex{
			Scratch: sr.scratch,
			Index:   sr.index,
		})
	}

	return results, nil
}
