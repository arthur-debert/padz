module github.com/arthur-debert/padz

go 1.23

require (
	github.com/adrg/xdg v0.5.3
	github.com/arthur-debert/nanostore v0.13.1
	github.com/arthur-debert/nanostore/types v0.0.0-00010101000000-000000000000
	github.com/charmbracelet/lipgloss v1.1.0
	github.com/charmbracelet/x/term v0.2.1
	github.com/dustin/go-humanize v1.0.1
	github.com/rs/zerolog v1.31.0
	github.com/spf13/cobra v1.10.1
	github.com/stretchr/testify v1.10.0
)

require (
	github.com/arthur-debert/nanostore/nanostore/ids v0.0.0-00010101000000-000000000000 // indirect
	github.com/aymanbagabas/go-osc52/v2 v2.0.1 // indirect
	github.com/charmbracelet/colorprofile v0.2.3-0.20250311203215-f60798e515dc // indirect
	github.com/charmbracelet/x/ansi v0.8.0 // indirect
	github.com/charmbracelet/x/cellbuf v0.0.13-0.20250311204145-2c3ea96c31dd // indirect
	github.com/cpuguy83/go-md2man/v2 v2.0.6 // indirect
	github.com/davecgh/go-spew v1.1.1 // indirect
	github.com/gofrs/flock v0.12.1 // indirect
	github.com/google/uuid v1.6.0 // indirect
	github.com/inconshreveable/mousetrap v1.1.0 // indirect
	github.com/lucasb-eyer/go-colorful v1.2.0 // indirect
	github.com/mattn/go-colorable v0.1.13 // indirect
	github.com/mattn/go-isatty v0.0.20 // indirect
	github.com/mattn/go-runewidth v0.0.16 // indirect
	github.com/muesli/termenv v0.16.0 // indirect
	github.com/pmezard/go-difflib v1.0.0 // indirect
	github.com/rivo/uniseg v0.4.7 // indirect
	github.com/russross/blackfriday/v2 v2.1.0 // indirect
	github.com/spf13/pflag v1.0.9 // indirect
	github.com/xo/terminfo v0.0.0-20220910002029-abceb7e1c41e // indirect
	golang.org/x/sys v0.30.0 // indirect
	gopkg.in/yaml.v3 v3.0.1 // indirect
)

replace github.com/arthur-debert/nanostore => ./local/third-parties/nanostore

replace github.com/arthur-debert/nanostore/types => ./local/third-parties/nanostore/types

replace github.com/arthur-debert/nanostore/nanostore/ids => ./local/third-parties/nanostore/nanostore/ids
