package logging

import (
	"bytes"
	"strings"
	"testing"

	"github.com/rs/zerolog"
	"github.com/rs/zerolog/log"
)

func TestSetupLogger_LogLevels(t *testing.T) {
	// Note: With dual logging (console + file), the global level is always Trace
	// so that the file gets all logs. Console filtering happens at the writer level.
	tests := []struct {
		name      string
		verbosity int
	}{
		{
			name:      "verbosity 0 - warn level console",
			verbosity: 0,
		},
		{
			name:      "verbosity 1 - info level console",
			verbosity: 1,
		},
		{
			name:      "verbosity 2 - debug level console",
			verbosity: 2,
		},
		{
			name:      "verbosity 3 - trace level console",
			verbosity: 3,
		},
		{
			name:      "verbosity 10 - trace level console (high values default to trace)",
			verbosity: 10,
		},
		{
			name:      "negative verbosity - trace level console (default case)",
			verbosity: -1,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			SetupLogger(tt.verbosity)

			// With dual logging, global level is always Trace so file gets everything
			actualLevel := zerolog.GlobalLevel()
			expectedGlobalLevel := zerolog.TraceLevel
			if actualLevel != expectedGlobalLevel {
				t.Errorf("expected global log level %v (for file logging), got %v", expectedGlobalLevel, actualLevel)
			}
		})
	}
}

func TestSetupLogger_Output(t *testing.T) {
	var buf bytes.Buffer
	oldLogger := log.Logger

	SetupLogger(1)

	logger := zerolog.New(&buf).With().Timestamp().Logger()
	log.Logger = logger

	log.Info().Msg("test message")

	log.Logger = oldLogger

	output := buf.String()
	if !strings.Contains(output, "test message") {
		t.Errorf("expected log output to contain 'test message', got: %s", output)
	}
}

func TestSetupLogger_CallerInfo(t *testing.T) {
	tests := []struct {
		name             string
		verbosity        int
		shouldHaveCaller bool
	}{
		{
			name:             "verbosity 0 - no caller info",
			verbosity:        0,
			shouldHaveCaller: false,
		},
		{
			name:             "verbosity 1 - no caller info",
			verbosity:        1,
			shouldHaveCaller: false,
		},
		{
			name:             "verbosity 2 - has caller info",
			verbosity:        2,
			shouldHaveCaller: true,
		},
		{
			name:             "verbosity 3 - has caller info",
			verbosity:        3,
			shouldHaveCaller: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var buf bytes.Buffer
			oldLogger := log.Logger

			SetupLogger(tt.verbosity)

			// Create a test logger with the same configuration but capturing to buffer
			logger := zerolog.New(&buf).With().Timestamp().Logger()
			if tt.verbosity >= 2 {
				logger = logger.With().Caller().Logger()
			}

			logger.Debug().Msg("test debug message")

			log.Logger = oldLogger

			output := buf.String()
			hasCaller := strings.Contains(output, "logging_test.go") || strings.Contains(output, "caller")

			if tt.shouldHaveCaller && !hasCaller {
				t.Errorf("expected caller info in output but didn't find it: %s", output)
			}
			if !tt.shouldHaveCaller && hasCaller {
				t.Errorf("didn't expect caller info but found it: %s", output)
			}
		})
	}
}

func TestGetLogger(t *testing.T) {
	tests := []struct {
		name          string
		componentName string
	}{
		{
			name:          "simple component name",
			componentName: "store",
		},
		{
			name:          "compound component name",
			componentName: "commands.create",
		},
		{
			name:          "empty component name",
			componentName: "",
		},
		{
			name:          "component name with special characters",
			componentName: "test-component_v2",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var buf bytes.Buffer
			oldLogger := log.Logger

			baseLogger := zerolog.New(&buf).With().Timestamp().Logger()
			log.Logger = baseLogger

			logger := GetLogger(tt.componentName)

			logger.Info().Msg("test message")

			log.Logger = oldLogger

			output := buf.String()
			if !strings.Contains(output, "test message") {
				t.Errorf("expected log output to contain 'test message', got: %s", output)
			}
			if tt.componentName != "" && !strings.Contains(output, tt.componentName) {
				t.Errorf("expected log output to contain component name '%s', got: %s", tt.componentName, output)
			}
		})
	}
}

func TestWithFields(t *testing.T) {
	tests := []struct {
		name   string
		fields map[string]interface{}
	}{
		{
			name: "single string field",
			fields: map[string]interface{}{
				"key1": "value1",
			},
		},
		{
			name: "multiple fields with different types",
			fields: map[string]interface{}{
				"string_field": "test",
				"int_field":    42,
				"bool_field":   true,
				"float_field":  3.14,
			},
		},
		{
			name:   "empty fields",
			fields: map[string]interface{}{},
		},
		{
			name: "nil value field",
			fields: map[string]interface{}{
				"nil_field": nil,
			},
		},
		{
			name: "complex nested field",
			fields: map[string]interface{}{
				"nested": map[string]string{
					"inner": "value",
				},
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			var buf bytes.Buffer
			oldLogger := log.Logger

			baseLogger := zerolog.New(&buf).With().Timestamp().Logger()
			log.Logger = baseLogger

			logger := WithFields(tt.fields)
			logger.Info().Msg("test message")

			log.Logger = oldLogger

			output := buf.String()
			if !strings.Contains(output, "test message") {
				t.Errorf("expected log output to contain 'test message', got: %s", output)
			}

			for key, value := range tt.fields {
				if value != nil && !strings.Contains(output, key) {
					t.Errorf("expected log output to contain field key '%s', got: %s", key, output)
				}
			}
		})
	}
}

func TestSetupLogger_Integration(t *testing.T) {
	var buf bytes.Buffer
	oldLogger := log.Logger

	SetupLogger(2)

	baseLogger := zerolog.New(&buf).With().Timestamp().Logger()
	log.Logger = baseLogger

	componentLogger := GetLogger("test-component")
	fieldsLogger := WithFields(map[string]interface{}{
		"request_id": "123456",
		"user_id":    789,
	})

	componentLogger.Debug().Msg("component debug message")
	fieldsLogger.Info().Msg("fields info message")
	log.Warn().Msg("global warning")

	log.Logger = oldLogger

	output := buf.String()

	if !strings.Contains(output, "component debug message") {
		t.Errorf("expected component debug message in output")
	}
	if !strings.Contains(output, "fields info message") {
		t.Errorf("expected fields info message in output")
	}
	if !strings.Contains(output, "global warning") {
		t.Errorf("expected global warning in output")
	}
}

func BenchmarkSetupLogger(b *testing.B) {
	for i := 0; i < b.N; i++ {
		SetupLogger(1)
	}
}

func BenchmarkGetLogger(b *testing.B) {
	SetupLogger(1)
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		GetLogger("benchmark-component")
	}
}

func BenchmarkWithFields(b *testing.B) {
	SetupLogger(1)
	fields := map[string]interface{}{
		"key1": "value1",
		"key2": 42,
		"key3": true,
	}
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		WithFields(fields)
	}
}

func TestLoggerConsistency(t *testing.T) {
	SetupLogger(2)

	logger1 := GetLogger("component1")
	logger2 := GetLogger("component1")

	var buf1, buf2 bytes.Buffer

	logger1 = logger1.Output(&buf1)
	logger2 = logger2.Output(&buf2)

	testMsg := "consistency test"
	logger1.Info().Msg(testMsg)
	logger2.Info().Msg(testMsg)

	output1 := buf1.String()
	output2 := buf2.String()

	if !strings.Contains(output1, testMsg) || !strings.Contains(output2, testMsg) {
		t.Errorf("both loggers should contain the test message")
	}

	if !strings.Contains(output1, "component1") || !strings.Contains(output2, "component1") {
		t.Errorf("both loggers should contain component name")
	}
}
