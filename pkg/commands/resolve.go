package commands

import (
	"fmt"
	"strings"

	"github.com/arthur-debert/padz/pkg/store"
)

// ResolveResult represents the result of resolving an ID
type ResolveResult struct {
	ID      string
	Scratch *store.Scratch
	Error   error
}

// ResolveMultipleIDs resolves multiple ID strings to scratches
// Returns a slice of scratches in the same order as the input IDs
// Returns an error if ANY ID is invalid (all or nothing validation)
func ResolveMultipleIDs(s *store.Store, all, global bool, project string, ids []string) ([]*store.Scratch, error) {
	if len(ids) == 0 {
		return []*store.Scratch{}, nil
	}

	// Track unique IDs to handle duplicates
	seen := make(map[string]bool)
	uniqueIDs := make([]string, 0, len(ids))

	// Preserve order but skip duplicates
	for _, id := range ids {
		if id == "" {
			continue
		}
		if !seen[id] {
			seen[id] = true
			uniqueIDs = append(uniqueIDs, id)
		}
	}

	// Resolve all IDs
	results := make([]*store.Scratch, 0, len(uniqueIDs))
	errors := make([]string, 0)

	for _, id := range uniqueIDs {
		scratch, err := ResolveScratchID(s, all, global, project, id)
		if err != nil {
			errors = append(errors, fmt.Sprintf("%s: %v", id, err))
			continue
		}
		results = append(results, scratch)
	}

	// If any errors occurred, return nil with combined error message
	if len(errors) > 0 {
		return nil, fmt.Errorf("failed to resolve IDs: %s", strings.Join(errors, "; "))
	}

	return results, nil
}

// ResolveMultipleIDsWithErrors resolves multiple ID strings to scratches
// Returns individual results with errors for each ID
// This allows partial success handling if needed
func ResolveMultipleIDsWithErrors(s *store.Store, all, global bool, project string, ids []string) []ResolveResult {
	results := make([]ResolveResult, 0, len(ids))
	seen := make(map[string]bool)

	for _, id := range ids {
		if id == "" {
			continue
		}

		// Handle duplicates by referencing the same scratch
		if seen[id] {
			// Find the previous result for this ID
			for _, r := range results {
				if r.ID == id && r.Error == nil {
					results = append(results, ResolveResult{
						ID:      id,
						Scratch: r.Scratch,
						Error:   nil,
					})
					break
				}
			}
			continue
		}

		seen[id] = true
		scratch, err := ResolveScratchID(s, all, global, project, id)
		results = append(results, ResolveResult{
			ID:      id,
			Scratch: scratch,
			Error:   err,
		})
	}

	return results
}

// ValidateIDs validates a slice of ID strings without actually resolving them
// Returns nil if all IDs are valid, or an error listing all invalid IDs
func ValidateIDs(ids []string) error {
	if len(ids) == 0 {
		return nil
	}

	invalidIDs := make([]string, 0)

	for _, id := range ids {
		if id == "" {
			invalidIDs = append(invalidIDs, "(empty)")
			continue
		}

		// Validate format
		if err := validateIDFormat(id); err != nil {
			invalidIDs = append(invalidIDs, fmt.Sprintf("%s (%v)", id, err))
		}
	}

	if len(invalidIDs) > 0 {
		return fmt.Errorf("invalid IDs: %s", strings.Join(invalidIDs, ", "))
	}

	return nil
}

// validateIDFormat checks if an ID has a valid format
func validateIDFormat(id string) error {
	if id == "" {
		return fmt.Errorf("empty ID")
	}

	// Check for deleted index (d1, d2, etc)
	if len(id) > 1 && id[0] == 'd' {
		if _, err := parseIndex(id[1:]); err != nil {
			return fmt.Errorf("invalid deleted index format")
		}
		return nil
	}

	// Check for pinned index (p1, p2, etc)
	if len(id) > 1 && id[0] == 'p' {
		if _, err := parseIndex(id[1:]); err != nil {
			return fmt.Errorf("invalid pinned index format")
		}
		return nil
	}

	// Check for regular index (1, 2, 3, etc)
	// First check if it looks like a number
	allDigits := true
	for _, c := range id {
		if c < '0' || c > '9' {
			allDigits = false
			break
		}
	}

	if allDigits {
		if _, err := parseIndex(id); err != nil {
			return fmt.Errorf("invalid index: %v", err)
		}
		return nil
	}

	// Otherwise, it should be a hash prefix (at least 1 character)
	if len(id) < 1 {
		return fmt.Errorf("hash prefix too short")
	}

	// Hash prefixes should only contain valid hex characters
	for _, c := range id {
		if !isHexChar(c) {
			return fmt.Errorf("invalid hash character: %c", c)
		}
	}

	return nil
}

// parseIndex attempts to parse a string as a positive integer index
func parseIndex(s string) (int, error) {
	if s == "" {
		return 0, fmt.Errorf("empty index")
	}

	index := 0
	for _, c := range s {
		if c < '0' || c > '9' {
			return 0, fmt.Errorf("non-numeric character")
		}
		index = index*10 + int(c-'0')
		if index > 1000000 { // Reasonable upper bound
			return 0, fmt.Errorf("index too large")
		}
	}

	if index < 1 {
		return 0, fmt.Errorf("index must be positive")
	}

	return index, nil
}

// isHexChar checks if a rune is a valid hexadecimal character
func isHexChar(c rune) bool {
	return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F')
}
