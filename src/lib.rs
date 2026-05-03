#![expect(
    clippy::field_reassign_with_default,
    clippy::if_same_then_else,
    clippy::len_without_is_empty,
    clippy::manual_clamp,
    clippy::needless_range_loop,
    clippy::nonminimal_bool,
    clippy::only_used_in_recursion,
    clippy::overly_complex_bool_expr,
    clippy::search_is_some,
    clippy::should_implement_trait,
    clippy::too_many_arguments,
    clippy::type_complexity
)]

pub mod beatmaps;
pub mod converter;
pub mod hud;
pub mod intro;
pub mod modes;
pub mod parser;
pub mod renderer;
pub mod results;
pub mod types;
pub mod utils;
pub mod video;
