// This file is managed by `cargo dev rename_lint`. Prefer using that when possible.

#[rustfmt::skip]
pub static RENAMED_LINTS: &[(&str, &str)] = &[
    ("clippy::blacklisted_name", "clippy::disallowed_names"),
    ("clippy::block_in_if_condition_expr", "clippy::blocks_in_if_conditions"),
    ("clippy::block_in_if_condition_stmt", "clippy::blocks_in_if_conditions"),
    ("clippy::box_vec", "clippy::box_collection"),
    ("clippy::const_static_lifetime", "clippy::redundant_static_lifetimes"),
    ("clippy::cyclomatic_complexity", "clippy::cognitive_complexity"),
    ("clippy::disallowed_method", "clippy::disallowed_methods"),
    ("clippy::disallowed_type", "clippy::disallowed_types"),
    ("clippy::eval_order_dependence", "clippy::mixed_read_write_in_expression"),
    ("clippy::for_loop_over_option", "clippy::for_loops_over_fallibles"),
    ("clippy::for_loop_over_result", "clippy::for_loops_over_fallibles"),
    ("clippy::identity_conversion", "clippy::useless_conversion"),
    ("clippy::if_let_some_result", "clippy::match_result_ok"),
    ("clippy::logic_bug", "clippy::overly_complex_bool_expr"),
    ("clippy::new_without_default_derive", "clippy::new_without_default"),
    ("clippy::option_and_then_some", "clippy::bind_instead_of_map"),
    ("clippy::option_expect_used", "clippy::expect_used"),
    ("clippy::option_map_unwrap_or", "clippy::map_unwrap_or"),
    ("clippy::option_map_unwrap_or_else", "clippy::map_unwrap_or"),
    ("clippy::option_unwrap_used", "clippy::unwrap_used"),
    ("clippy::ref_in_deref", "clippy::needless_borrow"),
    ("clippy::result_expect_used", "clippy::expect_used"),
    ("clippy::result_map_unwrap_or_else", "clippy::map_unwrap_or"),
    ("clippy::result_unwrap_used", "clippy::unwrap_used"),
    ("clippy::single_char_push_str", "clippy::single_char_add_str"),
    ("clippy::stutter", "clippy::module_name_repetitions"),
    ("clippy::to_string_in_display", "clippy::recursive_format_impl"),
    ("clippy::zero_width_space", "clippy::invisible_characters"),
    ("clippy::drop_bounds", "drop_bounds"),
    ("clippy::into_iter_on_array", "array_into_iter"),
    ("clippy::invalid_atomic_ordering", "invalid_atomic_ordering"),
    ("clippy::invalid_ref", "invalid_value"),
    ("clippy::mem_discriminant_non_enum", "enum_intrinsics_non_enums"),
    ("clippy::panic_params", "non_fmt_panics"),
    ("clippy::temporary_cstring_as_ptr", "temporary_cstring_as_ptr"),
    ("clippy::unknown_clippy_lints", "unknown_lints"),
    ("clippy::unused_label", "unused_labels"),
];
