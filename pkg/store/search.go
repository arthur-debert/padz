package store

import (
	"strings"
)

// SearchResult represents a search result with content included
type SearchResult struct {
	*Pad
	Content string
}

// Search returns pads that contain the search term
func (s *Store) Search(term string) ([]*SearchResult, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	var results []*SearchResult
	lowerTerm := strings.ToLower(term)

	for id, pad := range s.metadata.Pads {
		// Load content to search
		content, err := s.loadContent(id)
		if err != nil {
			// Skip pads we can't read
			continue
		}

		// Search in title and content
		titleMatch := strings.Contains(strings.ToLower(pad.Title), lowerTerm)
		contentMatch := strings.Contains(strings.ToLower(content), lowerTerm)

		if titleMatch || contentMatch {
			results = append(results, &SearchResult{
				Pad:     pad,
				Content: content,
			})
		}
	}

	// Sort by UserID (newest first)
	if len(results) > 1 {
		for i := 0; i < len(results)-1; i++ {
			for j := i + 1; j < len(results); j++ {
				if results[i].UserID < results[j].UserID {
					results[i], results[j] = results[j], results[i]
				}
			}
		}
	}

	return results, nil
}
