package utils

import (
	"testing"
	"time"
)

func TestParseDuration(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected time.Duration
		wantErr  bool
	}{
		// Standard Go durations
		{"standard hours", "24h", 24 * time.Hour, false},
		{"standard minutes", "30m", 30 * time.Minute, false},
		{"standard combined", "1h30m", 90 * time.Minute, false},

		// Custom day format
		{"single day", "1d", 24 * time.Hour, false},
		{"multiple days", "7d", 7 * 24 * time.Hour, false},
		{"fractional days", "1.5d", 36 * time.Hour, false},

		// Custom week format
		{"single week", "1w", 7 * 24 * time.Hour, false},
		{"multiple weeks", "2w", 14 * 24 * time.Hour, false},

		// Custom month format (30 days)
		{"single month", "1mo", 30 * 24 * time.Hour, false},
		{"multiple months", "3mo", 90 * 24 * time.Hour, false},

		// Custom year format (365 days)
		{"single year", "1y", 365 * 24 * time.Hour, false},

		// Empty string
		{"empty string", "", 0, false},

		// Error cases
		{"invalid format", "7days", 0, true},
		{"no number", "d", 0, true},
		{"invalid unit", "7x", 0, true},
		{"spaces", "7 d", 0, true},
		{"negative", "-7d", 0, true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := ParseDuration(tt.input)
			if (err != nil) != tt.wantErr {
				t.Errorf("ParseDuration(%q) error = %v, wantErr %v", tt.input, err, tt.wantErr)
				return
			}
			if got != tt.expected {
				t.Errorf("ParseDuration(%q) = %v, want %v", tt.input, got, tt.expected)
			}
		})
	}
}

func TestFormatDuration(t *testing.T) {
	tests := []struct {
		name     string
		duration time.Duration
		expected string
	}{
		{"zero", 0, "0s"},
		{"less than minute", 30 * time.Second, "< 1m"},
		{"exactly one minute", time.Minute, "1m"},
		{"multiple minutes", 45 * time.Minute, "45m"},
		{"one hour", time.Hour, "1h"},
		{"hours and minutes", 90 * time.Minute, "1h 30m"},
		{"one day", 24 * time.Hour, "1d"},
		{"days and hours", 30 * time.Hour, "1d 6h"},
		{"multiple days", 72 * time.Hour, "3d"},
		{"week", 7 * 24 * time.Hour, "7d"},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := FormatDuration(tt.duration)
			if got != tt.expected {
				t.Errorf("FormatDuration(%v) = %v, want %v", tt.duration, got, tt.expected)
			}
		})
	}
}
