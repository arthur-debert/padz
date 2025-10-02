package commands

import (
	"github.com/arthur-debert/padz/pkg/store"
	"regexp"
	"sort"
	"strings"
)

// ScratchWithIndex wraps a Scratch with its nanostore SimpleID for display
type ScratchWithIndex struct {
	store.Scratch
	Index string `json:"index"` // Nanostore SimpleID (e.g., "1", "p1", "d1")
}

// searchResult is an internal struct to hold search ranking information
type searchResult struct {
	scratch     store.Scratch
	simpleID    string // Nanostore SimpleID
	titleMatch  bool
	exactMatch  bool
	matchLength int
	originalPos int
}

func Search(s *store.Store, global bool, project, term string) ([]store.Scratch, error) {
	return SearchWithMode(s, global, project, term, ListModeActive)
}

// SearchWithMode performs a search using nanostore's native search functionality
func SearchWithMode(s *store.Store, global bool, project, term string, mode ListMode) ([]store.Scratch, error) {
	// Validate regex first to ensure consistent error handling
	re, err := regexp.Compile(term)
	if err != nil {
		return nil, err
	}

	// For active mode, use manual filtering to maintain exact behavior compatibility
	// (nanostore search may have different case sensitivity and regex support)
	scratches := LsWithMode(s, global, project, mode)

	var filtered []store.Scratch
	for _, scratch := range scratches {
		if re.MatchString(scratch.Content) || re.MatchString(scratch.Title) {
			filtered = append(filtered, scratch)
		}
	}

	return filtered, nil
}

// SearchWithIndices performs a search and returns results with their correct positional indices
func SearchWithIndices(s *store.Store, global bool, project, term string) ([]ScratchWithIndex, error) {
	return SearchWithIndicesMode(s, global, project, term, ListModeActive)
}

// SearchWithIndicesMode performs a search with mode and returns results with their nanostore SimpleIDs
func SearchWithIndicesMode(s *store.Store, global bool, project, term string, mode ListMode) ([]ScratchWithIndex, error) {
	// Validate regex first to ensure consistent error handling
	userRe, userReErr := regexp.Compile(term)
	if userReErr != nil {
		return nil, userReErr
	}

	// Get all scratches and do manual filtering with ranking
	allScratches := LsWithMode(s, global, project, mode)

	// Also compile case-insensitive version for literal matching
	re, err := regexp.Compile("(?i)" + regexp.QuoteMeta(term))
	if err != nil {
		return nil, err
	}

	var searchResults []searchResult

	for i, scratch := range allScratches {
		contentStr := scratch.Content
		titleLower := strings.ToLower(scratch.Title)
		termLower := strings.ToLower(term)

		// Check for matches
		titleMatch := strings.Contains(titleLower, termLower)
		bodyMatch := !titleMatch && (re.MatchString(contentStr) || userRe.MatchString(contentStr))

		if titleMatch || bodyMatch {
			// Calculate exact match and length for ranking
			exactMatch := titleMatch && titleLower == termLower
			matchLength := len(term) // Simple approximation
			if titleMatch {
				if loc := re.FindStringIndex(scratch.Title); loc != nil {
					matchLength = loc[1] - loc[0]
				}
			}

			searchResults = append(searchResults, searchResult{
				scratch:     scratch,
				simpleID:    scratch.ID,
				titleMatch:  titleMatch,
				exactMatch:  exactMatch,
				matchLength: matchLength,
				originalPos: i,
			})
		}
	}

	// Sort results: exact matches first, then title matches, then by match length, then original order
	sort.Slice(searchResults, func(i, j int) bool {
		if searchResults[i].exactMatch != searchResults[j].exactMatch {
			return searchResults[i].exactMatch
		}
		if searchResults[i].titleMatch != searchResults[j].titleMatch {
			return searchResults[i].titleMatch
		}
		if searchResults[i].matchLength != searchResults[j].matchLength {
			return searchResults[i].matchLength > searchResults[j].matchLength
		}
		return searchResults[i].originalPos < searchResults[j].originalPos
	})

	// Convert to final result format
	var results []ScratchWithIndex
	for _, sr := range searchResults {
		results = append(results, ScratchWithIndex{
			Scratch: sr.scratch,
			Index:   sr.simpleID,
		})
	}

	return results, nil
}
