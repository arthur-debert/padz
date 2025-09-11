package cli

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newRecoverCmd creates and returns a new recover command
func newRecoverCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "recover",
		Short: "Recover orphaned scratch files and clean metadata",
		Long: `Recover orphaned scratch files and clean metadata inconsistencies.

This command scans for:
- Orphaned files: Files on disk without metadata entries
- Missing files: Metadata entries without corresponding files

With appropriate flags, it can:
- Add orphaned files back to metadata
- Remove metadata entries for missing files
- Run in dry-run mode to preview changes`,
		Run: func(cmd *cobra.Command, args []string) {
			dryRun, _ := cmd.Flags().GetBool("dry-run")
			recoverOrphans, _ := cmd.Flags().GetBool("recover-orphans")
			cleanMissing, _ := cmd.Flags().GetBool("clean-missing")
			defaultProject, _ := cmd.Flags().GetString("project")
			global, _ := cmd.Flags().GetBool("global")

			// Get current directory
			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current working directory")
			}

			// Configure recovery options
			options := commands.RecoveryOptions{
				DryRun:         dryRun,
				RecoverOrphans: recoverOrphans,
				CleanMissing:   cleanMissing,
				DefaultProject: defaultProject,
			}

			// Log the operation mode
			if dryRun {
				log.Info().Msg("Running in dry-run mode - no changes will be made")
			}

			// Run recovery using StoreManager
			result, err := commands.RecoverWithStoreManager(dir, global, options)
			if err != nil {
				log.Fatal().Err(err).Msg("Recovery failed")
			}

			// Format and display results
			format, err := output.GetFormat(outputFormat)
			if err != nil {
				log.Fatal().Err(err).Msg("Invalid output format")
			}

			if format == output.JSONFormat {
				// JSON output
				if err := json.NewEncoder(os.Stdout).Encode(result); err != nil {
					log.Fatal().Err(err).Msg("Failed to encode JSON output")
				}
			} else {
				// Human-readable output
				printRecoveryResult(result, dryRun)
			}

			// Exit with error code if there were errors
			if len(result.Errors) > 0 {
				os.Exit(1)
			}
		},
	}

	// Add flags
	cmd.Flags().Bool("dry-run", true, "Preview changes without making them")
	cmd.Flags().Bool("recover-orphans", false, "Recover orphaned files by adding them to metadata")
	cmd.Flags().Bool("clean-missing", false, "Remove metadata entries for missing files")
	cmd.Flags().StringP("project", "p", "recovered", "Project name for recovered orphaned files")

	return cmd
}

// printRecoveryResult prints the recovery result in human-readable format
func printRecoveryResult(result *commands.RecoveryResult, dryRun bool) {
	// Print summary header
	fmt.Println("Recovery Analysis Complete")
	fmt.Println("==========================")
	fmt.Printf("Duration: %s\n", result.Summary.Duration)
	fmt.Println()

	// Print orphaned files
	if len(result.OrphanedFiles) > 0 {
		fmt.Printf("Orphaned Files (found on disk without metadata): %d\n", len(result.OrphanedFiles))
		fmt.Println("----------------------------------------------------")
		for _, orphan := range result.OrphanedFiles {
			fmt.Printf("  ID: %s\n", orphan.ID)
			fmt.Printf("  Title: %s\n", orphan.Title)
			fmt.Printf("  Size: %d bytes\n", orphan.Size)
			fmt.Printf("  Modified: %s\n", orphan.ModTime.Format("2006-01-02 15:04:05"))
			if orphan.Preview != "" {
				fmt.Printf("  Preview:\n")
				// Indent preview lines
				lines := splitLines(orphan.Preview)
				for i, line := range lines {
					if i < 3 { // Limit preview to 3 lines
						fmt.Printf("    %s\n", line)
					}
				}
			}
			fmt.Println()
		}
	}

	// Print missing files
	if len(result.MissingFiles) > 0 {
		fmt.Printf("Missing Files (metadata without files): %d\n", len(result.MissingFiles))
		fmt.Println("----------------------------------------")
		for _, missing := range result.MissingFiles {
			fmt.Printf("  ID: %s\n", missing.ID)
			fmt.Printf("  Title: %s\n", missing.Title)
			fmt.Printf("  Project: %s\n", missing.Project)
			fmt.Printf("  Created: %s\n", missing.CreatedAt.Format("2006-01-02 15:04:05"))
			fmt.Println()
		}
	}

	// Print recovered files
	if len(result.RecoveredFiles) > 0 {
		fmt.Printf("Recovered Files: %d\n", len(result.RecoveredFiles))
		fmt.Println("------------------")
		for _, recovered := range result.RecoveredFiles {
			fmt.Printf("  ID: %s\n", recovered.ID)
			fmt.Printf("  Title: %s\n", recovered.Title)
			fmt.Printf("  Project: %s\n", recovered.Project)
			fmt.Printf("  Source: %s\n", recovered.Source)
			fmt.Println()
		}
	}

	// Print errors
	if len(result.Errors) > 0 {
		fmt.Printf("Errors Encountered: %d\n", len(result.Errors))
		fmt.Println("---------------------")
		for _, err := range result.Errors {
			fmt.Printf("  Type: %s\n", err.Type)
			fmt.Printf("  Message: %s\n", err.Message)
			if err.FileID != "" {
				fmt.Printf("  File ID: %s\n", err.FileID)
			}
			fmt.Println()
		}
	}

	// Print action summary
	fmt.Println("Summary")
	fmt.Println("-------")
	fmt.Printf("  Orphaned files found: %d\n", result.Summary.TotalOrphaned)
	fmt.Printf("  Missing files found: %d\n", result.Summary.TotalMissing)
	if !dryRun {
		fmt.Printf("  Files recovered: %d\n", result.Summary.TotalRecovered)
	}
	fmt.Printf("  Errors: %d\n", result.Summary.TotalErrors)

	if dryRun {
		fmt.Println()
		fmt.Println("This was a dry run. No changes were made.")
		fmt.Println("To apply changes, run without --dry-run flag.")
	}
}

// splitLines splits a string into lines
func splitLines(s string) []string {
	var lines []string
	start := 0
	for i := 0; i < len(s); i++ {
		if s[i] == '\n' {
			lines = append(lines, s[start:i])
			start = i + 1
		}
	}
	if start < len(s) {
		lines = append(lines, s[start:])
	}
	return lines
}
