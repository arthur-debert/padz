package main

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/internal/version"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/logging"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

var (
	verbosity int
	verbose   bool
	debug     bool

	rootCmd = &cobra.Command{
		Use:   "padz",
		Short: "A simple command-line note-taking tool",
		Long: `padz is a simple shell command to create, search, view, and edit quick notes.
It uses the user's default command-line editor for note creation and editing,
and focuses on streamlined content management.`,
		PersistentPreRun: func(cmd *cobra.Command, args []string) {
			// Setup logging based on verbosity
			logging.SetupLogger(verbosity)
			log.Debug().Str("command", cmd.Name()).Msg("Command started")
		},
		Run: func(cmd *cobra.Command, args []string) {
			s, err := store.NewStore()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to initialize store")
			}

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get working directory")
			}

			proj, err := project.GetCurrentProject(dir)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current project")
			}

			content := commands.ReadContentFromPipe()
			if err := commands.Create(s, proj, content); err != nil {
				log.Fatal().Err(err).Msg("Failed to create note")
			}
		},
	}
)

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main(). It only needs to happen once to the rootCmd.
func Execute() error {
	return rootCmd.Execute()
}

func init() {
	// Verbosity flag for logging
	rootCmd.PersistentFlags().CountVarP(&verbosity, "verbose", "v", "Increase verbosity (-v, -vv, -vvv)")
	rootCmd.PersistentFlags().BoolVar(&verbose, "verbose-old", false, "Enable verbose output")
	rootCmd.PersistentFlags().BoolVar(&debug, "debug", false, "Enable debug output")

	// Version command
	rootCmd.AddCommand(&cobra.Command{
		Use:   "version",
		Short: "Print the version number",
		Long:  `Print the version number of padz`,
		Run: func(cmd *cobra.Command, args []string) {
			fmt.Printf("padz version %s (commit: %s, built: %s)\n", 
				version.Version, version.Commit, version.Date)
		},
	})

	// Completion command
	rootCmd.AddCommand(&cobra.Command{
		Use:   "completion [bash|zsh|fish|powershell]",
		Short: "Generate shell completion script",
		Long: `To load completions:

Bash:
  $ source <(padz completion bash)
  # To load completions for each session, execute once:
  # Linux:
  $ padz completion bash > /etc/bash_completion.d/padz
  # macOS:
  $ padz completion bash > /usr/local/etc/bash_completion.d/padz

Zsh:
  # If shell completion is not already enabled in your environment,
  # you will need to enable it.  You can execute the following once:
  $ echo "autoload -U compinit; compinit" >> ~/.zshrc
  # To load completions for each session, execute once:
  $ padz completion zsh > "${fpath[1]}/_padz"
  # You will need to start a new shell for this setup to take effect.

Fish:
  $ padz completion fish | source
  # To load completions for each session, execute once:
  $ padz completion fish > ~/.config/fish/completions/padz.fish

PowerShell:
  PS> padz completion powershell | Out-String | Invoke-Expression
  # To load completions for every new session, run:
  PS> padz completion powershell > padz.ps1
  # and source this file from your PowerShell profile.
`,
		DisableFlagsInUseLine: true,
		ValidArgs:             []string{"bash", "zsh", "fish", "powershell"},
		Args:                  cobra.MatchAll(cobra.ExactArgs(1), cobra.OnlyValidArgs),
		Run: func(cmd *cobra.Command, args []string) {
			switch args[0] {
			case "bash":
				cmd.Root().GenBashCompletion(cmd.OutOrStdout())
			case "zsh":
				cmd.Root().GenZshCompletion(cmd.OutOrStdout())
			case "fish":
				cmd.Root().GenFishCompletion(cmd.OutOrStdout(), true)
			case "powershell":
				cmd.Root().GenPowerShellCompletionWithDesc(cmd.OutOrStdout())
			}
		},
	})
}