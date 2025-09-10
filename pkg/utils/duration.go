package utils

import (
	"fmt"
	"regexp"
	"strconv"
	"strings"
	"time"
)

// ParseDuration parses duration strings like "7d", "2w", "30h", "1mo"
// It first tries standard Go duration parsing, then falls back to custom formats
func ParseDuration(s string) (time.Duration, error) {
	// Empty string means no duration
	if s == "" {
		return 0, nil
	}

	// Try standard duration first (e.g., "1h30m", "24h")
	if d, err := time.ParseDuration(s); err == nil {
		return d, nil
	}

	// Handle custom formats with regex
	s = strings.TrimSpace(s)
	re := regexp.MustCompile(`^(\d+(?:\.\d+)?)(d|w|mo|y)$`)
	matches := re.FindStringSubmatch(s)

	if len(matches) != 3 {
		return 0, fmt.Errorf("invalid duration format: %s (use formats like 7d, 2w, 1mo, 1y, or standard Go durations like 24h)", s)
	}

	// Parse the numeric part
	n, err := strconv.ParseFloat(matches[1], 64)
	if err != nil {
		return 0, fmt.Errorf("invalid number in duration: %s", matches[1])
	}

	// Convert to duration based on unit
	var d time.Duration
	switch matches[2] {
	case "d":
		d = time.Duration(n * 24 * float64(time.Hour))
	case "w":
		d = time.Duration(n * 7 * 24 * float64(time.Hour))
	case "mo":
		d = time.Duration(n * 30 * 24 * float64(time.Hour))
	case "y":
		d = time.Duration(n * 365 * 24 * float64(time.Hour))
	default:
		return 0, fmt.Errorf("unknown duration unit: %s", matches[2])
	}

	return d, nil
}

// FormatDuration formats a duration in a human-friendly way
func FormatDuration(d time.Duration) string {
	if d == 0 {
		return "0s"
	}

	days := d / (24 * time.Hour)
	d = d % (24 * time.Hour)
	hours := d / time.Hour
	d = d % time.Hour
	minutes := d / time.Minute

	var parts []string

	if days > 0 {
		parts = append(parts, fmt.Sprintf("%dd", days))
	}
	if hours > 0 {
		parts = append(parts, fmt.Sprintf("%dh", hours))
	}
	if minutes > 0 && days == 0 { // Only show minutes if less than a day
		parts = append(parts, fmt.Sprintf("%dm", minutes))
	}

	if len(parts) == 0 {
		// Less than a minute
		return "< 1m"
	}

	return strings.Join(parts, " ")
}
