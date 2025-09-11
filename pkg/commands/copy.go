package commands

import (
	"github.com/arthur-debert/padz/pkg/clipboard"
	"github.com/arthur-debert/padz/pkg/store"
)

// CopyMultiple copies multiple scratches to the clipboard
func CopyMultiple(s *store.Store, all bool, global bool, project string, ids []string) (int, error) {
	// Use simple aggregation without headers for clipboard
	options := DefaultAggregateOptions()
	options.Separator = DefaultSeparators.Clipboard

	aggregated, err := AggregateScratchContentsByIDs(s, all, global, project, ids, options)
	if err != nil {
		return 0, err
	}

	content := aggregated.GetCombinedContent()
	if err := clipboard.Copy([]byte(content)); err != nil {
		return 0, err
	}

	return len(aggregated.Scratches), nil
}

// Copy retrieves a scratch by index and copies its content to the clipboard
func Copy(s *store.Store, all bool, global bool, project string, indexStr string) error {
	_, err := CopyMultiple(s, all, global, project, []string{indexStr})
	return err
}

// CopyMultipleWithStoreManager copies multiple scratches to clipboard using StoreManager approach
func CopyMultipleWithStoreManager(workingDir string, globalFlag bool, ids []string) (int, error) {
	// Use simple aggregation without headers for clipboard
	options := DefaultAggregateOptions()
	options.Separator = DefaultSeparators.Clipboard

	aggregated, err := AggregateScratchContentsByIDsWithStoreManager(workingDir, globalFlag, ids, options)
	if err != nil {
		return 0, err
	}

	content := aggregated.GetCombinedContent()
	if err := clipboard.Copy([]byte(content)); err != nil {
		return 0, err
	}

	return len(aggregated.Scratches), nil
}

// CopyWithStoreManager retrieves a scratch by index and copies its content to the clipboard using StoreManager approach
func CopyWithStoreManager(workingDir string, globalFlag bool, indexStr string) error {
	_, err := CopyMultipleWithStoreManager(workingDir, globalFlag, []string{indexStr})
	return err
}
