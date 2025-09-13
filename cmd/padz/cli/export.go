package cli

import (
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
	"os"
)

// newExportCmd creates and returns a new export command
func newExportCmd() *cobra.Command {
	var format string

	cmd := &cobra.Command{
		Use:   "export [id...]",
		Short: "Export scratches to files",
		Long: `Export scratches to files in the specified format.

If no IDs are provided, all scratches in the current project are exported.
You can specify multiple IDs to export specific scratches.

Files are exported to a directory named padz-export-YYYY-MM-DD-HH-mm
with filenames in the format: <index>-<title>.<extension>

Examples:
  padz export                    # Export all scratches as txt
  padz export --format markdown  # Export all scratches as markdown
  padz export 1 2 3             # Export specific scratches
  padz export p1 p2             # Export pinned scratches`,
		Run: func(cmd *cobra.Command, args []string) {
			global, _ := cmd.Flags().GetBool("global")

			s, err := store.NewStoreWithScope(global)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to initialize store")
			}

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current working directory")
			}

			proj, err := project.GetCurrentProject(dir)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current project")
			}

			// Run discovery before exporting
			if err := s.RunDiscoveryBeforeCommand(); err != nil {
				log.Warn().Err(err).Msg("Failed to run discovery")
			}

			// Export scratches
			if err := commands.Export(s, global, proj, args, format); err != nil {
				log.Fatal().Err(err).Msg("Failed to export scratches")
			}
		},
	}

	cmd.Flags().StringVar(&format, "format", "txt", "Export format (txt or markdown)")

	return cmd
}
