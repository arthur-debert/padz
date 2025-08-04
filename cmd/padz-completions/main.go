package main

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/cmd/padz/cli"
)

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintf(os.Stderr, "Usage: %s <bash|zsh|fish|powershell>\n", os.Args[0])
		os.Exit(1)
	}

	shell := os.Args[1]
	rootCmd := cli.NewRootCmd()

	var err error
	switch shell {
	case "bash":
		err = rootCmd.GenBashCompletion(os.Stdout)
	case "zsh":
		err = rootCmd.GenZshCompletion(os.Stdout)
	case "fish":
		err = rootCmd.GenFishCompletion(os.Stdout, true)
	case "powershell":
		err = rootCmd.GenPowerShellCompletionWithDesc(os.Stdout)
	default:
		fmt.Fprintf(os.Stderr, "Unknown shell: %s\n", shell)
		fmt.Fprintf(os.Stderr, "Supported shells: bash, zsh, fish, powershell\n")
		os.Exit(1)
	}

	if err != nil {
		fmt.Fprintf(os.Stderr, "Error generating %s completion: %v\n", shell, err)
		os.Exit(1)
	}
}