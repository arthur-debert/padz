package cli

import (
	"fmt"
	"path/filepath"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/arthur-debert/padz/pkg/symlinks"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newLinksCmd creates and returns a new links command
func newLinksCmd() *cobra.Command {
	var linkDir string

	cmd := &cobra.Command{
		Use:   "links",
		Short: "Manage filesystem links to pads",
		Long: `Create and manage symlinks to pad files, allowing you to access them as regular files.

By default, creates links in ~/.padz-links/. You can then:
  cat ~/.padz-links/1           # Read pad by index
  cat ~/.padz-links/<id>        # Read pad by ID
  cat ~/.padz-links/<title>     # Read pad by title

Example:
  padz links                    # Update links in default location
  padz links -d /tmp/padz       # Update links in custom location`,
		Run: func(cmd *cobra.Command, args []string) {
			s, err := store.NewStore()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to initialize store")
			}

			manager, err := symlinks.NewManager(s, linkDir)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to create symlink manager")
			}

			if err := manager.Update(); err != nil {
				log.Fatal().Err(err).Msg("Failed to update symlinks")
			}

			dir := manager.GetLinkDir()
			fmt.Printf("✓ Symlinks updated in %s\n", dir)

			// Show example usage
			scratches := s.GetScratches()
			if len(scratches) > 0 {
				fmt.Println("\nExample usage:")
				fmt.Printf("  cat %s\n", filepath.Join(dir, "1"))
				if scratches[0].Title != "" && scratches[0].Title != "Untitled" {
					fmt.Printf("  cat %s\n", filepath.Join(dir, symlinks.SanitizeFilename(scratches[0].Title)))
				}
			}
		},
	}

	cmd.Flags().StringVarP(&linkDir, "directory", "d", "", "Directory to create symlinks in (default: ~/.padz-links)")

	return cmd
}
