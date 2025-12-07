use console::Style;
use once_cell::sync::Lazy;
use outstanding::{rgb_to_ansi256, Styles};

pub static LIST_STYLES: Lazy<Styles> = Lazy::new(|| {
    Styles::new()
        .missing_indicator("(!?)")
        .add("index_pinned", Style::new().yellow())
        .add("index_deleted", Style::new().red())
        .add("index_regular", Style::new())
        .add(
            "time",
            Style::new()
                .color256(rgb_to_ansi256((154, 154, 154)))
                .italic(),
        )
});

pub static FULL_PAD_STYLES: Lazy<Styles> = Lazy::new(|| {
    Styles::new()
        .missing_indicator("(!?)")
        .add("fp_index", Style::new().yellow())
        .add("fp_title", Style::new().bold())
});

pub static TEXT_LIST_STYLES: Lazy<Styles> = Lazy::new(|| Styles::new().missing_indicator(""));
