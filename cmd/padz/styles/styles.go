package styles

import (
	_ "embed"
	"encoding/json"
	"fmt"

	"github.com/charmbracelet/lipgloss"
)

//go:embed styles.json
var stylesJSON []byte

type StyleDefinition struct {
	Foreground string `json:"foreground"`
	Background string `json:"background"`
	Bold       bool   `json:"bold"`
	Italic     bool   `json:"italic"`
	Underline  bool   `json:"underline"`
}

type StyleSet struct {
	Primary    StyleDefinition `json:"primary"`
	Secondary  StyleDefinition `json:"secondary"`
	Accent     StyleDefinition `json:"accent"`
	Title      StyleDefinition `json:"title"`
	Body       StyleDefinition `json:"body"`
	Muted      StyleDefinition `json:"muted"`
	Error      StyleDefinition `json:"error"`
	Success    StyleDefinition `json:"success"`
	Warning    StyleDefinition `json:"warning"`
	PadID      StyleDefinition `json:"padId"`
	PadTitle   StyleDefinition `json:"padTitle"`
	PadProject StyleDefinition `json:"padProject"`
	PadTime    StyleDefinition `json:"padTime"`
}

type Styles struct {
	Dark  StyleSet `json:"dark"`
	Light StyleSet `json:"light"`
}

type StyleManager struct {
	styles       Styles
	activeStyles map[string]lipgloss.Style
	isDark       bool
}

var Manager *StyleManager

func Init() error {
	var styles Styles
	if err := json.Unmarshal(stylesJSON, &styles); err != nil {
		return fmt.Errorf("failed to unmarshal styles: %w", err)
	}

	Manager = &StyleManager{
		styles:       styles,
		activeStyles: make(map[string]lipgloss.Style),
		isDark:       lipgloss.HasDarkBackground(),
	}

	Manager.loadActiveStyles()
	return nil
}

func (sm *StyleManager) loadActiveStyles() {
	var set StyleSet
	if sm.isDark {
		set = sm.styles.Dark
	} else {
		set = sm.styles.Light
	}

	sm.activeStyles["primary"] = createStyle(set.Primary)
	sm.activeStyles["secondary"] = createStyle(set.Secondary)
	sm.activeStyles["accent"] = createStyle(set.Accent)
	sm.activeStyles["title"] = createStyle(set.Title)
	sm.activeStyles["body"] = createStyle(set.Body)
	sm.activeStyles["muted"] = createStyle(set.Muted)
	sm.activeStyles["error"] = createStyle(set.Error)
	sm.activeStyles["success"] = createStyle(set.Success)
	sm.activeStyles["warning"] = createStyle(set.Warning)
	sm.activeStyles["padId"] = createStyle(set.PadID)
	sm.activeStyles["padTitle"] = createStyle(set.PadTitle)
	sm.activeStyles["padProject"] = createStyle(set.PadProject)
	sm.activeStyles["padTime"] = createStyle(set.PadTime)
}

func createStyle(def StyleDefinition) lipgloss.Style {
	style := lipgloss.NewStyle()

	if def.Foreground != "" {
		style = style.Foreground(lipgloss.Color(def.Foreground))
	}
	if def.Background != "" {
		style = style.Background(lipgloss.Color(def.Background))
	}
	if def.Bold {
		style = style.Bold(true)
	}
	if def.Italic {
		style = style.Italic(true)
	}
	if def.Underline {
		style = style.Underline(true)
	}

	return style
}

func Get(name string) lipgloss.Style {
	if Manager == nil {
		return lipgloss.NewStyle()
	}
	if style, ok := Manager.activeStyles[name]; ok {
		return style
	}
	return lipgloss.NewStyle()
}

func SetDarkMode(isDark bool) {
	if Manager != nil {
		Manager.isDark = isDark
		Manager.loadActiveStyles()
	}
}

func IsDarkMode() bool {
	if Manager != nil {
		return Manager.isDark
	}
	return false
}