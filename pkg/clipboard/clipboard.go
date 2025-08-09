package clipboard

import (
	"bytes"
	"fmt"
	"os/exec"
	"runtime"
)

// Copy copies the given content to the system clipboard
func Copy(content []byte) error {
	switch runtime.GOOS {
	case "darwin":
		return copyDarwin(content)
	case "linux":
		return copyLinux(content)
	default:
		return fmt.Errorf("clipboard not supported on %s", runtime.GOOS)
	}
}

func copyDarwin(content []byte) error {
	cmd := exec.Command("pbcopy")
	cmd.Stdin = bytes.NewReader(content)
	return cmd.Run()
}

func copyLinux(content []byte) error {
	// Try xclip first, then xsel
	cmds := [][]string{
		{"xclip", "-selection", "clipboard"},
		{"xsel", "--clipboard", "--input"},
	}

	for _, cmdArgs := range cmds {
		cmd := exec.Command(cmdArgs[0], cmdArgs[1:]...)
		cmd.Stdin = bytes.NewReader(content)
		if err := cmd.Run(); err == nil {
			return nil
		}
	}

	return fmt.Errorf("no clipboard utility found (tried xclip and xsel)")
}
