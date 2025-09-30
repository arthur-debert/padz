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

// SearchWithMode performs a search with delete filtering options
func SearchWithMode(s *store.Store, global bool, project, term string, mode ListMode) ([]store.Scratch, error) {
	scratches := LsWithMode(s, global, project, mode)

	re, err := regexp.Compile(term)
	if err != nil {
		return nil, err
	}

	var filtered []store.Scratch
	for _, scratch := range scratches {
		// Content is now stored directly in the scratch
		if re.MatchString(scratch.Content) {
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
	// Get all scratches in the correct order
	allScratches := LsWithMode(s, global, project, mode)

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
		// Content is now stored directly in the scratch
		contentStr := scratch.Content
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
				simpleID:    scratch.ID, // Use the nanostore SimpleID directly
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
			Index:   sr.simpleID, // Use the nanostore SimpleID
		})
	}

	return results, nil
}
