package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra/doc"

	"github.com/arthur-debert/padz/cmd/padz/cli"
	"github.com/arthur-debert/padz/internal/version"
)

func main() {
	rootCmd := cli.NewRootCmd()

	header := &doc.GenManHeader{
		Title:   "PADZ",
		Section: "1",
		Source:  "padz " + version.Version,
		Manual:  "padz manual",
	}

	err := doc.GenMan(rootCmd, header, os.Stdout)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error generating man page: %v\n", err)
		os.Exit(1)
	}
}
