package formatter

import (
	"fmt"
	"io"
	"os"

	"github.com/arthur-debert/padz/cmd/padz/styles"
	"github.com/arthur-debert/padz/cmd/padz/templates"
	"github.com/arthur-debert/padz/pkg/store"
)

type TerminalFormatter struct {
	writer   io.Writer
	renderer *templates.Renderer
}

func NewTerminalFormatter(writer io.Writer) (*TerminalFormatter, error) {
	if writer == nil {
		writer = os.Stdout
	}

	if err := styles.Init(); err != nil {
		return nil, fmt.Errorf("failed to initialize styles: %w", err)
	}

	renderer, err := templates.NewRenderer()
	if err != nil {
		return nil, fmt.Errorf("failed to create renderer: %w", err)
	}

	return &TerminalFormatter{
		writer:   writer,
		renderer: renderer,
	}, nil
}

func (tf *TerminalFormatter) SetWriter(writer io.Writer) {
	tf.writer = writer
}

func (tf *TerminalFormatter) FormatList(scratches []store.Scratch, showProject bool) error {
	// Convert []store.Scratch to []*store.Scratch
	scratchPtrs := make([]*store.Scratch, len(scratches))
	for i := range scratches {
		scratchPtrs[i] = &scratches[i]
	}

	output, err := tf.renderer.RenderPadList(scratchPtrs, showProject)
	if err != nil {
		return err
	}
	_, err = fmt.Fprintln(tf.writer, output)
	return err
}

func (tf *TerminalFormatter) FormatError(err error) {
	errorStyle := styles.Get("error")
	fmt.Fprintln(os.Stderr, errorStyle.Render(err.Error()))
}

func (tf *TerminalFormatter) FormatSuccess(message string) {
	successStyle := styles.Get("success")
	_, _ = fmt.Fprintln(tf.writer, successStyle.Render(message))
}

func (tf *TerminalFormatter) FormatWarning(message string) {
	warningStyle := styles.Get("warning")
	_, _ = fmt.Fprintln(tf.writer, warningStyle.Render(message))
}

func (tf *TerminalFormatter) FormatString(content string) {
	_, _ = fmt.Fprint(tf.writer, content)
}

func (tf *TerminalFormatter) FormatPath(path string) {
	_, _ = fmt.Fprintln(tf.writer, path)
}

func (tf *TerminalFormatter) FormatContentView(content string) error {
	output, err := tf.renderer.RenderContentView(content)
	if err != nil {
		// Fallback to plain content
		_, _ = fmt.Fprint(tf.writer, content)
		return err
	}
	_, _ = fmt.Fprint(tf.writer, output)
	return nil
}

func (tf *TerminalFormatter) FormatContentPeek(startContent, endContent string, hasSkipped bool, skippedLines int) error {
	output, err := tf.renderer.RenderContentPeek(startContent, endContent, hasSkipped, skippedLines)
	if err != nil {
		// Fallback to basic peek format
		_, _ = fmt.Fprint(tf.writer, startContent)
		if hasSkipped {
			if _, err := fmt.Fprintf(tf.writer, "... %d more lines ...\n", skippedLines); err != nil {
				return err
			}
		}
		_, _ = fmt.Fprint(tf.writer, endContent)
		return err
	}
	_, _ = fmt.Fprint(tf.writer, output)
	return nil
}
