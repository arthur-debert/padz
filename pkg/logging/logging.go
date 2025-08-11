package logging

import (
	"bytes"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"time"

	"github.com/adrg/xdg"
	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
)

// SetupLogger configures the global logger with dual output: console (verbosity-controlled) + file (all levels)
func SetupLogger(verbosity int) {
	// Get log file path using XDG specification
	logFilePath, err := getLogFilePath()
	if err != nil {
		// Fallback: log to stderr only if we can't set up file logging
		setupConsoleOnlyLogging(verbosity)
		return
	}

	// Open log file for writing (create directories if needed)
	logDir := filepath.Dir(logFilePath)
	if err := os.MkdirAll(logDir, 0755); err != nil {
		setupConsoleOnlyLogging(verbosity)
		return
	}

	logFile, err := os.OpenFile(logFilePath, os.O_CREATE|os.O_WRONLY|os.O_APPEND, 0644)
	if err != nil {
		setupConsoleOnlyLogging(verbosity)
		return
	}

	// Determine console level based on verbosity
	var consoleLevel zerolog.Level
	switch verbosity {
	case 0:
		consoleLevel = zerolog.WarnLevel
	case 1:
		consoleLevel = zerolog.InfoLevel
	case 2:
		consoleLevel = zerolog.DebugLevel
	default:
		consoleLevel = zerolog.TraceLevel
	}

	// Create console writer with level filtering
	consoleWriter := &LevelFilterWriter{
		writer: zerolog.ConsoleWriter{
			Out:        os.Stderr,
			TimeFormat: time.RFC3339,
			NoColor:    false,
		},
		level: consoleLevel,
	}

	// File writer: logs everything (JSON format)
	fileWriter := logFile

	// Combine both writers
	multiWriter := io.MultiWriter(fileWriter, consoleWriter)

	// Set global level to Trace so file gets everything
	zerolog.SetGlobalLevel(zerolog.TraceLevel)

	// Create logger with both outputs
	log.Logger = zerolog.New(multiWriter).With().Timestamp().Logger()

	// Add caller information for debug and trace levels
	if verbosity >= 2 {
		log.Logger = log.Logger.With().Caller().Logger()
	}

	// Log the logging setup
	log.Debug().
		Int("verbosity", verbosity).
		Str("console_level", consoleLevel.String()).
		Str("log_file", logFilePath).
		Msg("Logger initialized with dual output")
}

// setupConsoleOnlyLogging sets up logging to stderr only (fallback)
func setupConsoleOnlyLogging(verbosity int) {
	// Configure zerolog based on verbosity
	switch verbosity {
	case 0:
		zerolog.SetGlobalLevel(zerolog.WarnLevel)
	case 1:
		zerolog.SetGlobalLevel(zerolog.InfoLevel)
	case 2:
		zerolog.SetGlobalLevel(zerolog.DebugLevel)
	default:
		zerolog.SetGlobalLevel(zerolog.TraceLevel)
	}

	// Configure console output with pretty printing
	output := zerolog.ConsoleWriter{
		Out:        os.Stderr,
		TimeFormat: time.RFC3339,
		NoColor:    false,
	}

	log.Logger = log.Output(output)

	// Add caller information for debug and trace levels
	if verbosity >= 2 {
		log.Logger = log.Logger.With().Caller().Logger()
	}

	// Log the logging level
	log.Debug().Int("verbosity", verbosity).Msg("Logger initialized (console only)")
}

// getLogFilePath returns the path for the log file using XDG specification and PKG_NAME env var
func getLogFilePath() (string, error) {
	// Get package name from environment variable, default to "padz"
	pkgName := os.Getenv("PKG_NAME")
	if pkgName == "" {
		pkgName = "padz"
	}

	// Use XDG state directory for logs (typically ~/.local/state/padz/)
	logDir, err := xdg.StateFile(filepath.Join(pkgName, fmt.Sprintf("%s.log", pkgName)))
	if err != nil {
		return "", fmt.Errorf("failed to get XDG state directory: %w", err)
	}

	return logDir, nil
}

// LevelFilterWriter wraps a writer and filters log entries based on level
type LevelFilterWriter struct {
	writer io.Writer
	level  zerolog.Level
}

// Write implements io.Writer interface with level filtering
func (w *LevelFilterWriter) Write(p []byte) (n int, err error) {
	// Parse the log level from the JSON log entry
	level, err := extractLogLevel(p)
	if err != nil {
		// If we can't parse level, write it anyway
		return w.writer.Write(p)
	}

	// Only write if level is >= our threshold level
	if level >= w.level {
		return w.writer.Write(p)
	}

	// If filtered out, pretend we wrote it (return success)
	return len(p), nil
}

// WriteLevel implements zerolog.LevelWriter interface
func (w *LevelFilterWriter) WriteLevel(level zerolog.Level, p []byte) (n int, err error) {
	if level >= w.level {
		if lw, ok := w.writer.(zerolog.LevelWriter); ok {
			return lw.WriteLevel(level, p)
		}
		return w.writer.Write(p)
	}
	return len(p), nil
}

// extractLogLevel extracts the log level from a JSON log entry
func extractLogLevel(p []byte) (zerolog.Level, error) {
	// Look for "level":"<level>" in the JSON
	levelStr := ""

	// Simple JSON parsing for level field
	if bytes.Contains(p, []byte(`"level":"trace"`)) {
		levelStr = "trace"
	} else if bytes.Contains(p, []byte(`"level":"debug"`)) {
		levelStr = "debug"
	} else if bytes.Contains(p, []byte(`"level":"info"`)) {
		levelStr = "info"
	} else if bytes.Contains(p, []byte(`"level":"warn"`)) {
		levelStr = "warn"
	} else if bytes.Contains(p, []byte(`"level":"error"`)) {
		levelStr = "error"
	} else if bytes.Contains(p, []byte(`"level":"fatal"`)) {
		levelStr = "fatal"
	} else if bytes.Contains(p, []byte(`"level":"panic"`)) {
		levelStr = "panic"
	}

	switch levelStr {
	case "trace":
		return zerolog.TraceLevel, nil
	case "debug":
		return zerolog.DebugLevel, nil
	case "info":
		return zerolog.InfoLevel, nil
	case "warn":
		return zerolog.WarnLevel, nil
	case "error":
		return zerolog.ErrorLevel, nil
	case "fatal":
		return zerolog.FatalLevel, nil
	case "panic":
		return zerolog.PanicLevel, nil
	default:
		return zerolog.InfoLevel, fmt.Errorf("unknown level")
	}
}

// GetLogger returns a contextualized logger with the given name
func GetLogger(name string) zerolog.Logger {
	return log.With().Str("component", name).Logger()
}

// WithFields returns a logger with additional fields
func WithFields(fields map[string]interface{}) zerolog.Logger {
	logger := log.Logger
	for k, v := range fields {
		logger = logger.With().Interface(k, v).Logger()
	}
	return logger
}
