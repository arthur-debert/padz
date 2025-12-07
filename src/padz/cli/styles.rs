use console::Style;
use once_cell::sync::Lazy;
use outstanding::{rgb_to_ansi256, Theme};

pub static PADZ_THEME: Lazy<Theme> = Lazy::new(|| {
    Theme::new()
        .add("index_pinned", Style::new().yellow())
        .add("index_deleted", Style::new().red())
        .add("index_regular", Style::new())
        .add(
            "time",
            Style::new()
                .color256(rgb_to_ansi256((154, 154, 154)))
                .italic(),
        )
        .add("fp_index", Style::new().yellow())
        .add("fp_title", Style::new().bold())
});
