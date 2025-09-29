package commands

import (
	"github.com/arthur-debert/padz/pkg/store"
)

// ViewMultiple views multiple scratches combined with headers
func ViewMultiple(s *store.Store, global bool, project string, ids []string) (string, error) {
	// Use aggregation with headers only for multiple items
	var options AggregateOptions
	if len(ids) > 1 {
		options = AggregateOptionsWithHeaders()
	} else {
		options = DefaultAggregateOptions()
	}

	aggregated, err := AggregateScratchContentsByIDs(s, global, project, ids, options)
	if err != nil {
		return "", err
	}

	if len(ids) > 1 {
		return aggregated.GetCombinedContentWithHeaders(), nil
	}
	return aggregated.GetCombinedContent(), nil
}

// View views a single scratch (wrapper for backward compatibility)
func View(s *store.Store, global bool, project string, indexStr string) (string, error) {
	return ViewMultiple(s, global, project, []string{indexStr})
}
