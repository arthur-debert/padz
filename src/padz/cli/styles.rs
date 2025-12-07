use console::Style;
use once_cell::sync::Lazy;
use outstanding::{ColorSpace, Styles};

pub static LIST_STYLES: Lazy<Styles> = Lazy::new(|| {
    Styles::new()
        .missing_indicator("(!?)")
        .add("index_pinned", Style::new().yellow())
        .add("index_deleted", Style::new().red())
        .add("index_regular", Style::new())
        .add_rgb("time", (, 154, 154), ColorSpace::Ansi256)
        .add("time", Style::new().italic())
});

pub static FULL_PAD_STYLES: Lazy<Styles> = Lazy::new(|| {
    Styles::new()
        .missing_indicator("(!?)")
        .add("fp_index", Style::new().yellow())
        .add("fp_title", Style::new().bold())
});

pub static TEXT_LIST_STYLES: Lazy<Styles> = Lazy::new(|| Styles::new().missing_indicator(""));
