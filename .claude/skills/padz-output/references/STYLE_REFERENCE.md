# Style Reference

All styles defined in `crates/padz/src/styles/default.yaml`.

## Semantic Styles (use in templates)

| Style Name | Light Mode | Dark Mode | Modifiers |
|------------|-----------|-----------|-----------|
| **Core** |
| `title` | Black | White | bold |
| `time` | #737373 | #B4B4B4 | italic |
| `hint` | #ADADAD | #6E6E6E | - |
| **List** |
| `list-index` | #C48C00 | #FFD60A | - |
| `list-title` | Black | White | - |
| `pinned` | #C48C00 | #FFD60A | bold |
| `deleted` | #BA212D | #FF8A80 | - |
| `deleted-index` | #BA212D | #FF8A80 | - |
| `deleted-title` | #737373 | #B4B4B4 | - |
| `status-icon` | #737373 | #B4B4B4 | - |
| **Search** |
| `highlight` | Black on #FFEB3B | Black on #E5B900 | - |
| `match` | Black on #FFEB3B | Black on #E5B900 | - |
| **Messages** |
| `error` | #BA212D | #FF8A80 | bold |
| `warning` | #C48C00 | #FFD60A | bold |
| `success` | #008000 | #90EE90 | - |
| `info` | #737373 | #B4B4B4 | - |
| **Help** |
| `help-header` | Black | White | bold |
| `help-section` | #C48C00 | #FFD60A | bold |
| `help-command` | #008000 | #90EE90 | - |
| `help-desc` | #737373 | #B4B4B4 | - |
| `help-usage` | Cyan | Cyan | - |
| **Template Content** |
| `help-text` | #ADADAD | #6E6E6E | - |
| `section-header` | #737373 | #B4B4B4 | - |
| `empty-message` | #737373 | #B4B4B4 | - |
| `preview` | #ADADAD | #6E6E6E | - |
| `truncation` | #737373 | #B4B4B4 | - |
| `line-number` | #737373 | #B4B4B4 | - |
| `separator` | #ADADAD | #6E6E6E | - |

## Internal Layers (do not use in templates)

### Visual Layer (Layer 1)

| Name | Light | Dark |
|------|-------|------|
| `_primary` | Black | White |
| `_gray` | #737373 | #B4B4B4 |
| `_gray_light` | #ADADAD | #6E6E6E |
| `_gold` | #C48C00 | #FFD60A |
| `_red` | #BA212D | #FF8A80 |
| `_green` | #008000 | #90EE90 |
| `_yellow_bg` | Black on #FFEB3B | Black on #E5B900 |

### Presentation Layer (Layer 2)

| Alias | Points To |
|-------|-----------|
| `_secondary` | `_gray` |
| `_tertiary` | `_gray_light` |
| `_accent` | `_gold` |
| `_danger` | `_red` |
| `_success` | `_green` |

## YAML Style Syntax

### Simple Alias
```yaml
my-style: _accent
```

### With Light/Dark Variants
```yaml
my-style:
  light:
    fg: [196, 140, 0]
  dark:
    fg: [255, 214, 10]
```

### With Modifiers
```yaml
my-style:
  bold: true
  italic: true
  light:
    fg: black
  dark:
    fg: white
```

### Color Formats
- Named: `black`, `white`, `red`, `green`, `blue`, `cyan`, `magenta`, `yellow`
- RGB array: `[196, 140, 0]`

### Available Modifiers
- `bold: true`
- `italic: true`
- `underline: true`
- `dim: true`

## Auto-Detection

Light/dark mode is auto-detected via the `dark-light` crate. The appropriate variant is selected at runtime based on terminal settings.
