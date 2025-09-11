package commands

import (
	"fmt"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
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

// SearchWithIndicesWithStoreManager performs a search across stores using StoreManager
func SearchWithIndicesWithStoreManager(workingDir string, globalFlag bool, searchTerm string, allFlag bool) ([]ScratchWithIndex, error) {
	sm := store.NewStoreManager()

	// Determine which stores to search
	storesToSearch := make(map[string]*store.Store)

	if allFlag {
		// Search in global store
		globalStore, err := sm.GetGlobalStore()
		if err != nil {
			return nil, fmt.Errorf("failed to get global store: %w", err)
		}
		storesToSearch["global"] = globalStore

		// Also search in current project store if available
		currentStore, scope, err := sm.GetCurrentStore(workingDir, false)
		if err == nil && scope != "global" {
			storesToSearch[scope] = currentStore
		}
	} else {
		// Search only in the current store (based on global flag)
		currentStore, scope, err := sm.GetCurrentStore(workingDir, globalFlag)
		if err != nil {
			return nil, fmt.Errorf("failed to get current store: %w", err)
		}
		storesToSearch[scope] = currentStore
	}

	// Try to compile as user regex first
	userRe, userReErr := regexp.Compile(searchTerm)
	if userReErr != nil {
		return nil, userReErr
	}

	// Also compile case-insensitive version for literal matching
	re, err := regexp.Compile("(?i)" + regexp.QuoteMeta(searchTerm))
	if err != nil {
		return nil, err
	}

	var allResults []ScratchWithIndex

	// Search in each store
	for scope, storeInstance := range storesToSearch {
		// Get all active scratches from this store
		scratches := storeInstance.GetScratches()
		var activeScratches []store.Scratch
		for _, s := range scratches {
			if !s.IsDeleted {
				activeScratches = append(activeScratches, s)
			}
		}

		// Sort by most recent first (consistent with LsWithMode)
		sort.Slice(activeScratches, func(i, j int) bool {
			return getMostRecentTime(&activeScratches[i]).After(getMostRecentTime(&activeScratches[j]))
		})

		// Separate pinned and unpinned
		var pinnedScratches []store.Scratch
		var unpinnedScratches []store.Scratch
		for _, scratch := range activeScratches {
			if scratch.IsPinned {
				pinnedScratches = append(pinnedScratches, scratch)
			} else {
				unpinnedScratches = append(unpinnedScratches, scratch)
			}
		}

		// Sort pinned by PinnedAt
		sort.Slice(pinnedScratches, func(i, j int) bool {
			return pinnedScratches[i].PinnedAt.After(pinnedScratches[j].PinnedAt)
		})

		// Combine pinned first, then unpinned
		orderedScratches := append(pinnedScratches, unpinnedScratches...)

		// Create index map for this store
		idToIndex := make(map[string]int)
		for i, scratch := range orderedScratches {
			idToIndex[scratch.ID] = i + 1
		}

		// Search through scratches
		var searchResults []searchResult
		for i, scratch := range orderedScratches {
			content, err := readScratchFile(scratch.ID)
			if err != nil {
				return nil, err
			}

			contentStr := string(content)
			titleLower := strings.ToLower(scratch.Title)
			termLower := strings.ToLower(searchTerm)

			// Check for matches
			titleMatch := false
			bodyMatch := false
			exactMatch := false
			matchLength := 0

			// Check title matches
			if strings.Contains(titleLower, termLower) {
				titleMatch = true
				if titleLower == termLower {
					exactMatch = true
				}
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

		// Convert to final result format with scope information
		for _, sr := range searchResults {
			result := ScratchWithIndex{
				Scratch: sr.scratch,
				Index:   sr.index,
			}

			// Set project information correctly
			if scope == "global" {
				result.Project = "global"
			} else if strings.HasPrefix(scope, "project:") {
				result.Project = strings.TrimPrefix(scope, "project:")
			}

			allResults = append(allResults, result)
		}
	}

	// If searching across multiple stores, we need to sort the combined results
	if len(storesToSearch) > 1 {
		// Re-sort all results by the same criteria
		sort.Slice(allResults, func(i, j int) bool {
			// First by scope (global first)
			if allResults[i].Project != allResults[j].Project {
				if allResults[i].Project == "global" {
					return true
				}
				if allResults[j].Project == "global" {
					return false
				}
			}

			// Then by search ranking (would need to preserve ranking info for proper sorting)
			// For now, just maintain the order from each store
			return false
		})

		// Re-assign indices for the combined result
		for i := range allResults {
			allResults[i].Index = i + 1
		}
	}

	return allResults, nil
}

// getMostRecentTime helper for sorting
func getMostRecentTime(scratch *store.Scratch) time.Time {
	mostRecent := scratch.CreatedAt
	if scratch.UpdatedAt.After(mostRecent) {
		mostRecent = scratch.UpdatedAt
	}
	if scratch.IsDeleted && scratch.DeletedAt != nil && scratch.DeletedAt.After(mostRecent) {
		mostRecent = *scratch.DeletedAt
	}
	return mostRecent
}
