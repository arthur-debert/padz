// Package logging provides a comprehensive dual logging system built with zerolog.
//
// # ARCHITECTURE
//
// The logging system uses two independent output streams:
//
// **Console Handler** (stderr):
//   - Human-readable format with colors and timestamps
//   - Respects verbosity flags: -v (Info), -vv (Debug), -vvv (Trace)
//   - Filtered output using custom LevelFilterWriter
//   - Intended for interactive debugging and development
//
// **File Handler** (persistent):
//   - JSON format with structured fields for programmatic parsing
//   - Logs ALL levels (Trace through Fatal) regardless of verbosity flags
//   - XDG-compliant locations:
//   - macOS: ~/Library/Application Support/padz/padz.log
//   - Linux: ~/.local/state/padz/padz.log
//   - Windows: %LOCALAPPDATA%\padz\padz.log
//   - Complete audit trail for troubleshooting and analysis
//
// # USAGE
//
// Initialize logging in main():
//
//	logging.SetupLogger(verbosity)
//
// Use structured logging throughout the application:
//
//	log.Info().Str("user", "john").Msg("User logged in")
//	log.Debug().Int("count", 42).Msg("Processing items")
//	log.Error().Err(err).Str("file", path).Msg("Failed to read file")
//
// Get component-specific loggers:
//
//	logger := logging.GetLogger("auth")
//	logger.Info().Msg("Authentication started")
//
// Add contextual fields:
//
//	logger := logging.WithFields(map[string]interface{}{
//	    "request_id": "123",
//	    "user_id":    456,
//	})
//
// # LOG LEVEL GUIDELINES
//
// **IMPORTANT**: Logging is for developers, not end users. User-facing messages
// should use the output package for proper formatting and internationalization.
//
// **FATAL**: Unrecoverable errors that cause immediate program termination
//   - Database connection failures
//   - Critical configuration errors
//   - Resource exhaustion
//
// **ERROR**: Recoverable errors that should be logged and handled
//   - File operation failures
//   - Network request timeouts
//   - Validation errors
//
// **WARN**: Unexpected conditions that don't prevent operation
//   - Deprecated API usage
//   - Performance degradation
//   - Missing optional configuration
//
// **INFO**: Entry points of major functions and significant state changes
//   - Command execution start/completion
//   - Authentication events
//   - Configuration loading
//   - Should provide clear execution flow understanding
//
// **DEBUG**: Detailed execution information including branches and conditions
//   - Function parameters and return values
//   - Loop iterations and conditional outcomes
//   - Intermediate calculations and transformations
//   - Include relevant data snippets (not full dumps)
//
// **TRACE**: Comprehensive application state information
//   - Complete configuration objects
//   - Full request/response payloads
//   - Detailed object dumps
//   - All data necessary to reproduce application state
//
// # STRUCTURED LOGGING
//
// Always use structured fields rather than string formatting:
//
// Good:
//
//	log.Info().Str("file", filename).Int("size", size).Msg("File processed")
//
// Bad:
//
//	log.Info().Msgf("File %s processed, size: %d", filename, size)
//
// Common field conventions:
//   - "component": Package or module name
//   - "operation": Current operation being performed
//   - "duration": Time taken (use .Dur() for time.Duration)
//   - "error": Error information (use .Err() for error type)
//   - "user_id", "request_id": Identifiers for tracing
//   - "file", "path": File system references
//
// # PERFORMANCE
//
// The logging system is designed for zero-allocation in production:
//   - Zerolog uses object pooling for events
//   - Console filtering happens at writer level
//   - File logging uses direct JSON marshaling
//   - Structured fields are more efficient than string formatting
//
// # TROUBLESHOOTING
//
// Find log file location:
//
//	padz ls -vv  # Look for "log_file" in debug output
//
// View recent logs:
//
//	tail -f ~/Library/Application Support/padz/padz.log | jq
//
// Filter logs by level:
//
//	jq 'select(.level=="error")' ~/Library/Application Support/padz/padz.log
//
// Extract errors with context:
//
//	jq 'select(.level=="error") | {time, message, error, file}' padz.log
package logging
