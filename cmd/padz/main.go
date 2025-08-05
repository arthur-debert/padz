package main

import (
	"os"

	"github.com/arthur-debert/padz/cmd/padz/cli"
)

func main() {
	if err := cli.Execute(); err != nil {
		os.Exit(1)
	}
}
