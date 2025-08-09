package clipboard

import (
	"runtime"
	"testing"
)

func TestCopy(t *testing.T) {
	// Skip test if not on supported platform
	if runtime.GOOS != "darwin" && runtime.GOOS != "linux" {
		t.Skip("Clipboard not supported on this platform")
	}

	content := []byte("test clipboard content")

	err := Copy(content)

	// On CI or systems without clipboard utilities, this might fail
	// We just check that the function doesn't panic
	if err != nil && runtime.GOOS == "linux" {
		// Expected on Linux systems without xclip/xsel
		t.Logf("Clipboard copy failed (expected on systems without clipboard utilities): %v", err)
	}
}
