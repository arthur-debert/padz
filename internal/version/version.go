// Package version provides version information for my-cli
package version

// Build-time variables set by ldflags
var (
	// Version is the semantic version
	Version = "dev"

	// Commit is the git commit SHA
	Commit = "unknown"

	// Date is the build date
	Date = "unknown"
)

// Info returns formatted version information
func Info() string {
	return Version + " (commit: " + Commit + ", built: " + Date + ")"
}
