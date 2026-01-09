//! # CLI Templates Module
//!
//! Out output pipleine, oustanding crate based, relies on templates for rendering term output.
//!
//! We have preference for stand-alone templates files, as seprateing them from code makes is easier
//! and safer to edit, diff and so on.
//!
//! We then include the template files as string constants here, so that they can be used as a regular
//! string literals elsewhere in the code.
//!
//! Templates are minijinja based. A few important best practices:
//!
//!     1. Blank Lines / Whitespace:
//!         
//!     While natural to keep templates organized as the output they produce, there are often times
//!     where that forces the template to become unreadble (i.e. many nested conditionals, very long
//!     lines). It can become quite tricky to iterate on blank lines and whitespaces, specilly when
//!     dealing with loops and conditionals.
//!     For this reason, we have templates requiring explicit line breaks, which make it clear where
//!     they are coming from.
//!     2. Reusability and Composition:
//!     Templates can and should be nested when appropriate. This allows for reuse (i.e. a pad
//!     listing title) that can be used in multiple places, and keeps templates smaller and more
//!     readable. Else we descend into a "god output" where everything is defined.
//!
//!     3. Judicial Conditionals:
//!     While conditionals are necessary, they can quickly make templates unreadable.
//!     They are best used when branching what gets output, but not when they contronling styles.
//!
//!     For example, {% if pad.is_pinned %} <pinned-style> {% else %} <regular-style> {% endif %}
//!     throughout various parts in the template.  In this case its best to set the style variable
//!     that does the logic, and then use the style variable directly.
//!
//!     4. Harder Logic
//!     While best avoided, for when more complex logic is needed, it is best to move that logic
//!     into the rust code, and pass the results as functions for the template to use.
//!
//!
//!     
//!
pub const LIST_TEMPLATE: &str = include_str!("templates/list.tmp");
pub const FULL_PAD_TEMPLATE: &str = include_str!("templates/full_pad.tmp");
pub const TEXT_LIST_TEMPLATE: &str = include_str!("templates/text_list.tmp");
pub const MESSAGES_TEMPLATE: &str = include_str!("templates/messages.tmp");
