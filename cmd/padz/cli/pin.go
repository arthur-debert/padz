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
		Use:     "pin <id> [id...]",
		Aliases: []string{"p"},
		Short:   "Pin one or more scratches to the top of the list (p)",
		Long: `Pin one or more scratches to the top of the list. Pinned scratches appear with a special
prefix (p1, p2, etc.) and are always shown first when listing scratches.

Maximum of 5 scratches can be pinned at once.

Examples:
  padz pin 1        # Pin a single scratch
  padz pin 1 3 5    # Pin multiple scratches`,
		Args: cobra.MinimumNArgs(1),
		Run:  runPin,
	}
}

func runPin(cmd *cobra.Command, args []string) {
	log.Debug().Msg("Starting pin command")

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

	st, err := store.NewStoreWithScope(global)
	if err != nil {
		log.Error().Err(err).Msg("Failed to create store")
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	// Pin multiple scratches
	pinnedTitles, err := commands.PinMultiple(st, global, proj, args)
	if err != nil {
		log.Error().Err(err).Msg("Failed to pin scratches")
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	// Show list in verbose mode
	ShowListAfterCommand(st, global, proj)

	// Show success message if not silent
	if !IsSilentMode() {
		if len(pinnedTitles) == 0 {
			fmt.Println("No scratches were pinned (already pinned)")
		} else if len(pinnedTitles) == 1 {
			fmt.Printf("Scratch \"%s\" pinned successfully\n", pinnedTitles[0])
		} else {
			fmt.Printf("%d scratches pinned successfully\n", len(pinnedTitles))
		}
	}
}

// newUnpinCmd creates the unpin command
func newUnpinCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "unpin <id> [id...]",
		Aliases: []string{"u"},
		Short:   "Unpin one or more scratches (u)",
		Long: `Unpin one or more scratches. The scratches will return to their normal position
in the chronological list.

You can use either the regular index, pinned index (p1, p2), or hash ID.

Examples:
  padz unpin p1        # Unpin a single scratch
  padz unpin p1 p2 3   # Unpin multiple scratches`,
		Args: cobra.MinimumNArgs(1),
		Run:  runUnpin,
	}
}

func runUnpin(cmd *cobra.Command, args []string) {
	log.Debug().Msg("Starting unpin command")

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

	st, err := store.NewStoreWithScope(global)
	if err != nil {
		log.Error().Err(err).Msg("Failed to create store")
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	// Unpin multiple scratches
	unpinnedTitles, err := commands.UnpinMultiple(st, global, proj, args)
	if err != nil {
		log.Error().Err(err).Msg("Failed to unpin scratches")
		fmt.Fprintf(os.Stderr, "Error: %v\n", err)
		os.Exit(1)
	}

	// Show list in verbose mode
	ShowListAfterCommand(st, global, proj)

	// Show success message if not silent
	if !IsSilentMode() {
		if len(unpinnedTitles) == 0 {
			fmt.Println("No scratches were unpinned (not pinned)")
		} else if len(unpinnedTitles) == 1 {
			fmt.Printf("Scratch \"%s\" unpinned successfully\n", unpinnedTitles[0])
		} else {
			fmt.Printf("%d scratches unpinned successfully\n", len(unpinnedTitles))
		}
	}
}
