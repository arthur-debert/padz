package editor

import (
	"os"
	"os/exec"
)

func OpenInEditor(content []byte) ([]byte, error) {
	editor := os.Getenv("EDITOR")
	if editor == "" {
		editor = "vim" // default to vim
	}

	tmpfile, err := os.CreateTemp("", "scratch-")
	if err != nil {
		return nil, err
	}
	defer os.Remove(tmpfile.Name())

	if len(content) > 0 {
		if _, err := tmpfile.Write(content); err != nil {
			return nil, err
		}
	}

	if err := tmpfile.Close(); err != nil {
		return nil, err
	}

	cmd := exec.Command(editor, tmpfile.Name())
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		return nil, err
	}

	return os.ReadFile(tmpfile.Name())
}
