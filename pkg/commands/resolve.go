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

// ResolveMultipleIDs resolves multiple ID strings to scratches using nanostore's bulk operations
// Returns a slice of scratches in the same order as the input IDs
// Returns an error if ANY ID is invalid (all or nothing validation)
//
// This function delegates all ID resolution logic to the store layer for better performance
// and consistency. All resolution patterns (SimpleIDs, UUIDs, prefixes, deleted indices)
// are handled by nanostore's ResolveBulkIDs method.
func ResolveMultipleIDs(s *store.Store, global bool, project string, ids []string) ([]*store.Scratch, error) {
	return s.ResolveBulkIDs(ids, project, global)
}

// ResolveMultipleIDsWithErrors resolves multiple ID strings to scratches
// Returns individual results with errors for each ID
// This allows partial success handling if needed
func ResolveMultipleIDsWithErrors(s *store.Store, global bool, project string, ids []string) []ResolveResult {
	results := make([]ResolveResult, 0, len(ids))
	seen := make(map[string]*store.Scratch)

	for _, id := range ids {
		if id == "" {
			continue
		}

		// Handle duplicates by referencing the same scratch
		if scratch, exists := seen[id]; exists {
			results = append(results, ResolveResult{
				ID:      id,
				Scratch: scratch,
				Error:   nil,
			})
			continue
		}

		// Use the store's single ID resolution method
		scratch, err := s.ResolveBulkIDs([]string{id}, project, global)
		if err != nil {
			results = append(results, ResolveResult{
				ID:      id,
				Scratch: nil,
				Error:   err,
			})
		} else {
			seen[id] = scratch[0]
			results = append(results, ResolveResult{
				ID:      id,
				Scratch: scratch[0],
				Error:   nil,
			})
		}
	}

	return results
}

// ValidateIDs validates a slice of ID strings by attempting to resolve them
// Returns nil if all IDs are valid, or an error listing all invalid IDs
//
// Note: This now delegates to the store layer which has comprehensive validation
// for all ID formats (SimpleIDs, UUIDs, prefixes, deleted indices, etc.)
func ValidateIDs(s *store.Store, global bool, project string, ids []string) error {
	_, err := s.ResolveBulkIDs(ids, project, global)
	return err
}

// ValidateIDsFormat provides format-only validation for testing (legacy function)
func ValidateIDsFormat(ids []string) error {
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

// Test helper functions (kept for backward compatibility with existing tests)

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
