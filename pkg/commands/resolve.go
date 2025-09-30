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

// ResolveMultipleIDs resolves multiple ID strings to scratches using atomic bulk operations
// Returns a slice of scratches in the same order as the input IDs
// Returns an error if ANY ID is invalid (all or nothing validation)
//
// This function fixes the ID instability issue by using a snapshot of all data
// for the entire batch resolution, preventing state changes between individual lookups.
func ResolveMultipleIDs(s *store.Store, global bool, project string, ids []string) ([]*store.Scratch, error) {
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

	// Create a consistent snapshot for all resolution operations
	// This prevents ID instability when state changes between individual lookups
	snapshot := createResolutionSnapshot(s, global, project)

	// Resolve all IDs against the stable snapshot using the same logic as ResolveScratchID
	results := make([]*store.Scratch, 0, len(uniqueIDs))
	errors := make([]string, 0)

	for _, id := range uniqueIDs {
		scratch, err := resolveIDFromSnapshotLikeOriginal(s, snapshot, id)
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

// ResolutionSnapshot contains stable data for consistent ID resolution
type ResolutionSnapshot struct {
	// All scratches for SimpleID/UUID resolution (global scope)
	allScratches []store.Scratch
	// ID maps for fast lookup
	simpleIDMap map[string]*store.Scratch
	uuidMap     map[string]*store.Scratch
	// Filtered scratches for scope-aware index resolution
	filteredScratches []store.Scratch
	// Context for filtering validation
	global  bool
	project string
}

// createResolutionSnapshot creates a stable snapshot of all data needed for ID resolution
func createResolutionSnapshot(s *store.Store, global bool, project string) *ResolutionSnapshot {
	// Get all scratches globally (for SimpleID/UUID resolution)
	allScratches := s.GetAllScratchesWithFilter("", false)   // All project scratches
	globalScratches := s.GetAllScratchesWithFilter("", true) // All global scratches

	// Combine all scratches
	allCombined := make([]store.Scratch, 0, len(allScratches)+len(globalScratches))
	allCombined = append(allCombined, allScratches...)
	allCombined = append(allCombined, globalScratches...)

	// Build lookup maps
	simpleIDMap := make(map[string]*store.Scratch)
	uuidMap := make(map[string]*store.Scratch)

	for i := range allCombined {
		scratch := &allCombined[i]
		simpleIDMap[scratch.ID] = scratch
		// Note: scratch.ID is the SimpleID, but we might also want UUID lookup
		// For now, we'll handle UUID resolution through the original nanostore logic
	}

	// Get filtered scratches for scope-aware operations (indexes, etc.)
	filteredScratches := s.GetAllScratchesWithFilter(project, global)

	// Ensure all scratches have consistent SimpleIDs in their ID field
	// In some environments, the store may return inconsistent ID formats
	for i := range filteredScratches {
		// Ensure this scratch has a SimpleID by looking it up in the main collection
		scratch := &filteredScratches[i]
		for j := range allCombined {
			if allCombined[j].Title == scratch.Title &&
				allCombined[j].CreatedAt.Equal(scratch.CreatedAt) &&
				allCombined[j].Project == scratch.Project {
				scratch.ID = allCombined[j].ID // Use the SimpleID from main collection
				break
			}
		}
	}

	return &ResolutionSnapshot{
		allScratches:      allCombined,
		simpleIDMap:       simpleIDMap,
		uuidMap:           uuidMap,
		filteredScratches: filteredScratches,
		global:            global,
		project:           project,
	}
}


// resolveIDFromSnapshotLikeOriginal mimics ResolveScratchID but uses snapshot data
// This ensures we get the same behavior (including correct SimpleIDs) while preventing ID instability
func resolveIDFromSnapshotLikeOriginal(s *store.Store, snapshot *ResolutionSnapshot, id string) (*store.Scratch, error) {
	// Handle deleted indexes (d1, d2, etc) - this doesn't use nanostore's ResolveUUID
	if len(id) > 1 && id[0] == 'd' {
		// Use the original GetScratchByIndex logic
		return GetScratchByIndex(s, snapshot.global, snapshot.project, id)
	}

	// For all other IDs, try to resolve using snapshot lookup first
	// This mimics the nanostore ResolveUUID call but uses our snapshot

	// Check if it's a numeric index first
	if index, err := parseIndex(id); err == nil {
		// Get active scratches from filtered scope (like original logic)
		var activeScratches []*store.Scratch
		for i := range snapshot.filteredScratches {
			if !snapshot.filteredScratches[i].IsDeleted {
				activeScratches = append(activeScratches, &snapshot.filteredScratches[i])
			}
		}

		if index < 1 || index > len(activeScratches) {
			return nil, fmt.Errorf("scratch not found: %s", id)
		}

		// Get the target scratch, then resolve it properly via GetScratchByUUID equivalent
		targetScratch := activeScratches[index-1]

		// Find the UUID for this scratch and get it via the proper path
		if uuid, err := s.ResolveIDToUUID(targetScratch.ID); err == nil {
			return s.GetScratchByUUID(uuid)
		}

		// Fallback to direct return if UUID resolution fails
		return targetScratch, nil
	}

	// Check for pinned index (p1, p2, etc)
	if len(id) > 1 && id[0] == 'p' {
		pinnedIndexStr := id[1:]
		pinnedIndex, err := parseIndex(pinnedIndexStr)
		if err != nil {
			return nil, fmt.Errorf("scratch not found: %s", id)
		}

		// Get pinned scratches from filtered scope
		var pinnedScratches []*store.Scratch
		for i := range snapshot.filteredScratches {
			if !snapshot.filteredScratches[i].IsDeleted && snapshot.filteredScratches[i].IsPinned {
				pinnedScratches = append(pinnedScratches, &snapshot.filteredScratches[i])
			}
		}

		if pinnedIndex < 1 || pinnedIndex > len(pinnedScratches) {
			return nil, fmt.Errorf("scratch not found: %s", id)
		}

		// Get the target scratch, then resolve it properly
		targetScratch := pinnedScratches[pinnedIndex-1]

		if uuid, err := s.ResolveIDToUUID(targetScratch.ID); err == nil {
			return s.GetScratchByUUID(uuid)
		}

		return targetScratch, nil
	}

	// For non-index IDs (SimpleIDs, UUIDs, hash prefixes), use the original ResolveUUID approach
	// Try to resolve the ID using nanostore's ResolveUUID (this handles SimpleIDs and full UUIDs)
	uuid, err := s.ResolveIDToUUID(id)
	if err != nil {
		// If it can't be resolved, it might be a partial UUID prefix
		// Try to find a scratch with matching UUID prefix using snapshot
		for i := range snapshot.allScratches {
			if strings.HasPrefix(snapshot.allScratches[i].ID, id) {
				// Apply project filtering
				scratch := &snapshot.allScratches[i]
				if snapshot.global && scratch.Project != "global" {
					continue
				}
				if !snapshot.global && snapshot.project != "" && scratch.Project != snapshot.project {
					continue
				}
				return scratch, nil
			}
		}
		return nil, fmt.Errorf("scratch not found: %s", id)
	}

	// Get the scratch by its UUID - this ensures correct SimpleID format
	scratch, err := s.GetScratchByUUID(uuid)
	if err != nil {
		return nil, err
	}

	// Verify it belongs to the correct project/scope (same as original ResolveScratchID)
	if snapshot.global && scratch.Project != "global" {
		return nil, fmt.Errorf("scratch not found in global scope: %s", id)
	}
	if !snapshot.global && snapshot.project != "" && scratch.Project != snapshot.project {
		return nil, fmt.Errorf("scratch not found in project %s: %s", snapshot.project, id)
	}

	return scratch, nil
}

// ResolveMultipleIDsWithErrors resolves multiple ID strings to scratches
// Returns individual results with errors for each ID
// This allows partial success handling if needed
func ResolveMultipleIDsWithErrors(s *store.Store, global bool, project string, ids []string) []ResolveResult {
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
		scratch, err := ResolveScratchID(s, global, project, id)
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
