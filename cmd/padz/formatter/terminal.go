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

func (tf *TerminalFormatter) FormatList(scratches []store.Scratch) error {
	// Convert []store.Scratch to []*store.Scratch
	scratchPtrs := make([]*store.Scratch, len(scratches))
	for i := range scratches {
		scratchPtrs[i] = &scratches[i]
	}
	
	output, err := tf.renderer.RenderPadList(scratchPtrs)
	if err != nil {
		return err
	}
	fmt.Fprintln(tf.writer, output)
	return nil
}

func (tf *TerminalFormatter) FormatError(err error) {
	errorStyle := styles.Get("error")
	fmt.Fprintln(os.Stderr, errorStyle.Render(err.Error()))
}

func (tf *TerminalFormatter) FormatSuccess(message string) {
	successStyle := styles.Get("success")
	fmt.Fprintln(tf.writer, successStyle.Render(message))
}

func (tf *TerminalFormatter) FormatWarning(message string) {
	warningStyle := styles.Get("warning")
	fmt.Fprintln(tf.writer, warningStyle.Render(message))
}

func (tf *TerminalFormatter) FormatString(content string) {
	fmt.Fprint(tf.writer, content)
}

func (tf *TerminalFormatter) FormatPath(path string) {
	fmt.Fprintln(tf.writer, path)
}