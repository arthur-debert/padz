package cli

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newPinCmd creates the pin command
func newPinCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "pin <id>",
		Aliases: []string{"p"},
		Short:   "Pin a scratch to the top of the list (p)",
		Long: `Pin a scratch to the top of the list. Pinned scratches appear with a special
prefix (p1, p2, etc.) and are always shown first when listing scratches.

Maximum of 5 scratches can be pinned at once.`,
		Args: cobra.ExactArgs(1),
		Run:  runPin,
	}
}

func runPin(cmd *cobra.Command, args []string) {
	log.Debug().Msg("Starting pin command")

	all, _ := cmd.Flags().GetBool("all")
	global, _ := cmd.Flags().GetBool("global")

	dir, err := os.Getwd()
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to get current working directory")
	}

	proj, err := project.GetCurrentProject(dir)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to get current project")
	}

	if global {
		proj = "global"
	}

	st, err := store.NewStore()
	if err != nil {
		log.Error().Err(err).Msg("Failed to create store")
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	// Get scratch details before pinning
	scratch, err := commands.GetScratchByIndex(st, all, global, proj, args[0])
	if err != nil {
		log.Error().Err(err).Str("id", args[0]).Msg("Failed to get scratch")
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	if err := commands.Pin(st, all, global, proj, args[0]); err != nil {
		log.Error().Err(err).Str("id", args[0]).Msg("Failed to pin scratch")
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	// Show list in verbose mode
	ShowListAfterCommand(st, all, global, proj)

	// Show success message if not silent
	if !IsSilentMode() {
		fmt.Printf("Scratch \"%s\" pinned successfully\n", scratch.Title)
	}
}

// newUnpinCmd creates the unpin command
func newUnpinCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "unpin <id>",
		Aliases: []string{"u"},
		Short:   "Unpin a scratch (u)",
		Long: `Unpin a scratch. The scratch will return to its normal position
in the chronological list.

You can use either the regular index, pinned index (p1, p2), or hash ID.`,
		Args: cobra.ExactArgs(1),
		Run:  runUnpin,
	}
}

func runUnpin(cmd *cobra.Command, args []string) {
	log.Debug().Msg("Starting unpin command")

	all, _ := cmd.Flags().GetBool("all")
	global, _ := cmd.Flags().GetBool("global")

	dir, err := os.Getwd()
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to get current working directory")
	}

	proj, err := project.GetCurrentProject(dir)
	if err != nil {
		log.Fatal().Err(err).Msg("Failed to get current project")
	}

	if global {
		proj = "global"
	}

	st, err := store.NewStore()
	if err != nil {
		log.Error().Err(err).Msg("Failed to create store")
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	// Get scratch details before unpinning
	scratch, err := commands.GetScratchByIndex(st, all, global, proj, args[0])
	if err != nil {
		log.Error().Err(err).Str("id", args[0]).Msg("Failed to get scratch")
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	if err := commands.Unpin(st, all, global, proj, args[0]); err != nil {
		log.Error().Err(err).Str("id", args[0]).Msg("Failed to unpin scratch")
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	// Show list in verbose mode
	ShowListAfterCommand(st, all, global, proj)

	// Show success message if not silent
	if !IsSilentMode() {
		fmt.Printf("Scratch \"%s\" unpinned successfully\n", scratch.Title)
	}
}
