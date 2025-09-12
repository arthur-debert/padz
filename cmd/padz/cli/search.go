package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newSearchCommand() *cobra.Command {
	var all bool

	cmd := &cobra.Command{
		Use:   "search [term]",
		Short: "Search for pads containing the specified term",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			searchTerm := args[0]

			if all {
				return searchAllScopes(searchTerm)
			}

			// Search current scope only
			dir, err := os.Getwd()
			if err != nil {
				return fmt.Errorf("failed to get current directory: %w", err)
			}

			scope, err := store.DetectScope(dir)
			if err != nil {
				return fmt.Errorf("failed to detect scope: %w", err)
			}

			return searchScope(scope, searchTerm)
		},
	}

	cmd.Flags().BoolVar(&all, "all", false, "Search pads from all scopes")

	return cmd
}

func searchScope(scope, searchTerm string) error {
	dispatcher := store.NewDispatcher()
	results, err := dispatcher.SearchPads(searchTerm, scope)
	if err != nil {
		return fmt.Errorf("failed to search pads: %w", err)
	}

	fmt.Printf("=== Search results for '%s' in scope '%s' ===\n\n", searchTerm, scope)
	if len(results) == 0 {
		fmt.Println("No matches found")
		return nil
	}

	for _, result := range results {
		title := result.Title
		if title == "" {
			title = "(untitled)"
		}
		explicitID := store.FormatExplicitID(scope, result.UserID)

		// Show a snippet of the matching content
		snippet := getContentSnippet(result.Content, searchTerm)
		fmt.Printf("%10s. %-30s %s\n", explicitID, title, result.CreatedAt.Format("2006-01-02 15:04"))
		if snippet != "" {
			fmt.Printf("           %s\n", snippet)
		}
		fmt.Println()
	}

	return nil
}

func searchAllScopes(searchTerm string) error {
	dispatcher := store.NewDispatcher()
	allResults, errors := dispatcher.SearchAllScopes(searchTerm)

	// Display results
	if len(allResults) == 0 {
		fmt.Printf("No matches found for '%s' in any scope\n", searchTerm)
		return nil
	}

	fmt.Printf("=== Search results for '%s' ===\n\n", searchTerm)
	first := true
	for scope, results := range allResults {
		if len(results) == 0 {
			continue
		}

		if !first {
			fmt.Println()
		}
		first = false

		fmt.Printf("--- Scope '%s' ---\n", scope)
		for _, result := range results {
			title := result.Title
			if title == "" {
				title = "(untitled)"
			}
			explicitID := store.FormatExplicitID(scope, result.UserID)

			snippet := getContentSnippet(result.Content, searchTerm)
			fmt.Printf("%10s. %-30s %s\n", explicitID, title, result.CreatedAt.Format("2006-01-02 15:04"))
			if snippet != "" {
				fmt.Printf("           %s\n", snippet)
			}
		}
	}

	// Display errors if any
	if len(errors) > 0 {
		fmt.Printf("\nEncountered %d error(s) while searching:\n", len(errors))
		for _, err := range errors {
			fmt.Printf("  - %v\n", err)
		}
	}

	return nil
}

func getContentSnippet(content, searchTerm string) string {
	lines := strings.Split(content, "\n")
	lowerTerm := strings.ToLower(searchTerm)

	for _, line := range lines {
		if strings.Contains(strings.ToLower(line), lowerTerm) {
			// Truncate long lines
			if len(line) > 60 {
				line = line[:60] + "..."
			}
			return strings.TrimSpace(line)
		}
	}

	return ""
}
