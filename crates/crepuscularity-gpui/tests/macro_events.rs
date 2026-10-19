use crepuscularity_gpui::view;
use gpui::{prelude::*, Context, KeyDownEvent, SharedString, Window};

#[test]
fn generated_lengths_reject_negative_sizes_and_preserve_signed_offsets() {
    let mut actual = view! {r#"div w-[20px] w-[-2px] h-[30px] h-[-25%] p-2 p-[-1rem] p-auto gap-2 gap-[-3px] gap-auto text-[14px] text-[-1px] text-[50%] m-[-4px] top-[-3px]"#};
    let mut expected = gpui::div()
        .w(gpui::px(20.0))
        .h(gpui::px(30.0))
        .p_2()
        .gap_2()
        .text_size(gpui::px(14.0))
        .m(gpui::px(-4.0))
        .top(gpui::px(-3.0));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn static_css_hex_colors_expand_and_preserve_alpha_in_generated_views() {
    let mut actual = view! {r#"div bg-[#123] bg-[#12] text-[#1234] text-[#xyz] border-[#aabbcc80] border-[#12345]"#};
    let mut expected = gpui::div()
        .bg(gpui::rgb(0x112233))
        .text_color(gpui::rgba(0x11223344))
        .border_color(gpui::rgba(0xaabbcc80));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn static_grid_utilities_construct_the_requested_gpui_style() {
    let mut actual = view! {r#"div grid grid-cols-2 grid-cols-0 grid-rows-3 grid-rows-0 col-span-2 col-span-0 row-span-2 row-span-0 col-start--1 col-start-0 row-start-2 row-start-0 col-end-3 col-end-0 row-end-4 row-end-0 items-stretch justify-evenly"#};
    let mut expected = gpui::div()
        .grid()
        .grid_cols(2)
        .grid_rows(3)
        .col_span(2)
        .row_span(2)
        .col_start(-1)
        .row_start(2)
        .col_end(3)
        .row_end(4)
        .items_stretch()
        .justify_evenly();
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn bound_ids_and_event_closures_compile_against_gpui() {
    let button_id = SharedString::from("bound-button");
    let _ = view! {r#"<button id={button_id} @click={|_, _, _| {}}>Press</button>"#};
    let _ = view! {r#"<div @hover={|_, _, _| {}}>Hover</div>"#};
    let _ = view! {r#"button @click={|_, _, _| {}}"#};
    let _ = KeyboardView::render;
}

struct KeyboardView;

impl KeyboardView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        view! {r#"<div @keydown|ctrl+s={handle_key} @keyup|cmd+Esc={|_, _, _| {}}>Key</div>"#}
    }

    fn handle_key(&mut self, _event: &KeyDownEvent, _window: &mut Window, _cx: &mut Context<Self>) {
    }
}

#[test]
fn dynamic_class_lists_compile_with_ids_and_handlers() {
    let theme = String::from("flex flex-row gap-4 p-2 bg-red-500");
    let _ = view! {r#"<div id="dynamic" class={theme} @click={|_, _, _| {}}>Content</div>"#};
    let theme = SharedString::from("flex flex-col");
    let _ = view! {r#"div class={theme}"#};
}

#[test]
fn multiple_state_utilities_construct_without_panicking() {
    let _ = view! {r#"div hover:bg-red-500 hover:text-white focus:p-2 focus:m-1 active:bg-blue-500 active:text-black"#};
}

#[test]
fn conditional_target_classes_construct() {
    let enabled = true;
    let _ = view! {r#"div class:gpui:flex-row={enabled} class:gui:p-2={enabled} class:web:hidden={enabled}"#};
}

#[test]
fn mixed_static_and_dynamic_state_classes_construct_and_evaluate_once() {
    let mut evaluations = 0;
    let _ = view! {r#"div hover:bg-blue-500 class={{ evaluations += 1; "hover:bg-red-500 hover:text-white focus:p-2 active:bg-black" }} hover:p-2"#};
    assert_eq!(evaluations, 1);
}

#[test]
fn arbitrary_tracking_generates_pixels_and_ignores_invalid_overrides() {
    let mut actual = view! {r#"div tracking-[-0.5rem] tracking-[50%] tracking-[auto] tracking-[1e38rem] tracking-[-1e38rem]"#};
    let mut expected = gpui::div().letter_spacing(gpui::px(-8.0));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn empty_font_utilities_preserve_the_generated_font_family() {
    let mut actual = view! {r#"div font-['Times_New_Roman'] font-[] font-[''] font-[___]"#};
    let mut expected = gpui::div().font_family("Times New Roman");
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn generated_self_alignment_matches_native_gpui_styles() {
    let cases = [
        (view! { "div self-start" }, gpui::div().self_start()),
        (view! { "div self-end" }, gpui::div().self_end()),
        (view! { "div self-center" }, gpui::div().self_center()),
        (view! { "div self-stretch" }, gpui::div().self_stretch()),
        (view! { "div self-baseline" }, gpui::div().self_baseline()),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let _ = view! { "div hover:self-center focus:self-end active:self-start" };
}

#[test]
fn generated_self_auto_clears_alignment_in_base_and_state_styles() {
    let mut actual = view! { "div self-center self-auto" };
    assert!(actual.style().align_self.is_none());
    let reset = true;
    let mut actual = view! { "div self-center class:self-auto={reset}" };
    assert!(actual.style().align_self.is_none());
    let reset = false;
    let mut actual = view! { "div self-center class:self-auto={reset}" };
    assert_eq!(actual.style().align_self, Some(gpui::AlignSelf::Center));
    let _ = view! { "div hover:self-center hover:self-auto focus:self-auto active:self-auto" };
    let _ = view! { "div web:self-auto" };
}

#[test]
fn generated_grid_full_spans_and_auto_lines_replace_prior_placement() {
    let cases = [
        (
            view! { "div col-span-2 col-span-full" },
            gpui::div().col_span_full(),
        ),
        (
            view! { "div row-span-2 row-span-full" },
            gpui::div().row_span_full(),
        ),
        (
            view! { "div col-start-2 col-start-auto" },
            gpui::div().col_start_auto(),
        ),
        (
            view! { "div col-end-3 col-end-auto" },
            gpui::div().col_end_auto(),
        ),
        (
            view! { "div row-start-2 row-start-auto" },
            gpui::div().row_start_auto(),
        ),
        (
            view! { "div row-end-3 row-end-auto" },
            gpui::div().row_end_auto(),
        ),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let full = true;
    let mut actual = view! { "div col-span-2 class:col-span-full={full}" };
    let mut expected = gpui::div().col_span_full();
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    let _ = view! { "div hover:col-span-full focus:row-span-full active:col-start-auto" };
}

#[test]
fn generated_aspect_ratios_validate_and_reset() {
    let cases = [
        (view! { "div aspect-square" }, Some(1.0)),
        (view! { "div aspect-video" }, Some(16.0 / 9.0)),
        (view! { "div aspect-[4/3]" }, Some(4.0 / 3.0)),
        (view! { "div aspect-[1.5]" }, Some(1.5)),
        (
            view! { "div aspect-square aspect-[0] aspect-[-4/-3] aspect-[1/0] aspect-[NaN] aspect-[1e38/1e-38]" },
            Some(1.0),
        ),
        (view! { "div aspect-square aspect-auto" }, None),
    ];
    for (mut actual, expected) in cases {
        assert_eq!(actual.style().aspect_ratio, expected);
    }
    let reset = true;
    let mut actual = view! { "div aspect-square class:aspect-auto={reset}" };
    assert!(actual.style().aspect_ratio.is_none());
    let _ =
        view! { "div hover:aspect-video hover:aspect-auto focus:aspect-[4/3] active:aspect-auto" };
}

#[test]
fn generated_arbitrary_border_widths_validate_without_shadowing_colors() {
    let mut actual = view! {r#"div border-[2px] border-[-1px] border-[50%] border-[auto] border-t-[0.5rem] border-b-[3px] border-l-[0px] border-r-[4px] border-[#1234]"#};
    let mut expected = gpui::div()
        .border(gpui::px(2.0))
        .border_t(gpui::rems(0.5))
        .border_b(gpui::px(3.0))
        .border_l(gpui::px(0.0))
        .border_r(gpui::px(4.0))
        .border_color(gpui::rgba(0x11223344));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    let enabled = true;
    let _ =
        view! { "div hover:border-[2px] focus:border-t-[0.5rem] class:border-r-[4px]={enabled}" };
}

#[test]
fn generated_arbitrary_radii_match_native_corner_styles() {
    let cases = [
        (
            view! { "div rounded-[2px] rounded-[-1px] rounded-[50%] rounded-[auto] rounded-[NaNpx]" },
            gpui::div().rounded(gpui::px(2.0)),
        ),
        (
            view! { "div rounded-t-[0.5rem]" },
            gpui::div().rounded_t(gpui::rems(0.5)),
        ),
        (
            view! { "div rounded-b-[3px]" },
            gpui::div().rounded_b(gpui::px(3.0)),
        ),
        (
            view! { "div rounded-l-[4px]" },
            gpui::div().rounded_l(gpui::px(4.0)),
        ),
        (
            view! { "div rounded-r-[5px]" },
            gpui::div().rounded_r(gpui::px(5.0)),
        ),
        (
            view! { "div rounded-tl-[6px]" },
            gpui::div().rounded_tl(gpui::px(6.0)),
        ),
        (
            view! { "div rounded-tr-[7px]" },
            gpui::div().rounded_tr(gpui::px(7.0)),
        ),
        (
            view! { "div rounded-bl-[8px]" },
            gpui::div().rounded_bl(gpui::px(8.0)),
        ),
        (
            view! { "div rounded-br-[0px]" },
            gpui::div().rounded_br(gpui::px(0.0)),
        ),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let enabled = true;
    let _ = view! { "div hover:rounded-[2px] focus:rounded-tl-[0.5rem] class:rounded-br-[4px]={enabled}" };
}

#[test]
fn generated_flex_basis_supports_lengths_fractions_and_auto() {
    let cases = [
        (
            view! { "div basis-4" },
            gpui::div().flex_basis(gpui::rems(1.0)),
        ),
        (
            view! { "div basis-1/2" },
            gpui::div().flex_basis(gpui::relative(0.5)),
        ),
        (
            view! { "div basis-[20px] basis-[-2px] basis-[-50%] basis-[NaNpx]" },
            gpui::div().flex_basis(gpui::px(20.0)),
        ),
        (
            view! { "div basis-[0.5rem]" },
            gpui::div().flex_basis(gpui::rems(0.5)),
        ),
        (
            view! { "div basis-4 basis-auto" },
            gpui::div().flex_basis(gpui::Length::Auto),
        ),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let enabled = true;
    let _ = view! { "div hover:basis-1/2 focus:basis-auto class:basis-[20px]={enabled}" };
}

#[test]
fn generated_arbitrary_line_heights_match_runtime_units_and_validation() {
    let cases = [
        (
            view! { "div leading-[20px] leading-[-1px] leading-[50%] leading-[auto] leading-[NaNpx]" },
            gpui::div().line_height(gpui::px(20.0)),
        ),
        (
            view! { "div leading-[1.25rem]" },
            gpui::div().line_height(gpui::rems(1.25)),
        ),
        (
            view! { "div leading-[0px]" },
            gpui::div().line_height(gpui::px(0.0)),
        ),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let enabled = true;
    let _ =
        view! { "div hover:leading-[20px] focus:leading-[1.25rem] class:leading-[24px]={enabled}" };
}

#[test]
fn generated_line_clamps_validate_counts_and_apply_overflow() {
    let mut actual = view! { "div line-clamp-3 line-clamp-0 line-clamp--1 line-clamp-1.5 line-clamp-184467440737095516160" };
    let mut expected = gpui::div().line_clamp(3);
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    let enabled = true;
    let mut actual = view! { "div line-clamp-3 class:line-clamp-2={enabled}" };
    let mut expected = gpui::div().line_clamp(2);
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    let _ = view! { "div hover:line-clamp-2 focus:line-clamp-3 active:line-clamp-4" };
}

#[test]
fn generated_font_styles_support_standard_aliases_and_resets() {
    let cases = [
        (view! { "div italic" }, gpui::div().italic()),
        (view! { "div font-italic" }, gpui::div().italic()),
        (view! { "div italic not-italic" }, gpui::div().not_italic()),
        (
            view! { "div font-bold font-extralight" },
            gpui::div().font_weight(gpui::FontWeight::EXTRA_LIGHT),
        ),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let enabled = true;
    let _ =
        view! { "div hover:italic focus:not-italic active:font-extralight class:italic={enabled}" };
}

#[test]
fn generated_flex_shorthands_and_reverse_layouts_replace_prior_values() {
    let cases = [
        (view! { "div flex-none flex-auto" }, gpui::div().flex_auto()),
        (
            view! { "div flex-1 flex-initial" },
            gpui::div().flex_initial(),
        ),
        (view! { "div flex-1 flex-none" }, gpui::div().flex_none()),
        (
            view! { "div flex-row flex-row-reverse" },
            gpui::div().flex_row_reverse(),
        ),
        (
            view! { "div flex-col flex-col-reverse" },
            gpui::div().flex_col_reverse(),
        ),
        (
            view! { "div flex-wrap flex-wrap-reverse" },
            gpui::div().flex_wrap_reverse(),
        ),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let enabled = true;
    let _ = view! { "div hover:flex-none focus:flex-auto active:flex-row-reverse class:flex-wrap-reverse={enabled}" };
}

#[test]
fn generated_directional_border_widths_preserve_other_edges() {
    let cases = [
        (
            view! { "div border-2 border-t-0" },
            gpui::div().border_2().border_t_0(),
        ),
        (
            view! { "div border-2 border-b-4" },
            gpui::div().border_2().border_b_4(),
        ),
        (
            view! { "div border-2 border-l-8" },
            gpui::div().border_2().border_l_8(),
        ),
        (
            view! { "div border-4 border-r-2 border-dashed" },
            gpui::div().border_4().border_r_2().border_dashed(),
        ),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let _ = view! { "div border-t-2 border-t-4 border-t-8 border-b-0 border-b-2 border-b-8 border-l-0 border-l-2 border-l-4 border-r-0 border-r-4 border-r-8 hover:border-t-0" };
}

#[test]
fn generated_named_corner_radii_match_the_complete_runtime_scale() {
    let cases = [
        (view! { "div rounded-t-none" }, gpui::div().rounded_t_none()),
        (view! { "div rounded-t-sm" }, gpui::div().rounded_t_sm()),
        (view! { "div rounded-t-md" }, gpui::div().rounded_t_md()),
        (view! { "div rounded-t-lg" }, gpui::div().rounded_t_lg()),
        (view! { "div rounded-t-xl" }, gpui::div().rounded_t_xl()),
        (view! { "div rounded-t-2xl" }, gpui::div().rounded_t_2xl()),
        (view! { "div rounded-t-3xl" }, gpui::div().rounded_t_3xl()),
        (view! { "div rounded-t-full" }, gpui::div().rounded_t_full()),
        (view! { "div rounded-t" }, gpui::div().rounded_t_md()),
        (view! { "div rounded-b-none" }, gpui::div().rounded_b_none()),
        (view! { "div rounded-b-sm" }, gpui::div().rounded_b_sm()),
        (view! { "div rounded-b-md" }, gpui::div().rounded_b_md()),
        (view! { "div rounded-b-lg" }, gpui::div().rounded_b_lg()),
        (view! { "div rounded-b-xl" }, gpui::div().rounded_b_xl()),
        (view! { "div rounded-b-2xl" }, gpui::div().rounded_b_2xl()),
        (view! { "div rounded-b-3xl" }, gpui::div().rounded_b_3xl()),
        (view! { "div rounded-b-full" }, gpui::div().rounded_b_full()),
        (view! { "div rounded-b" }, gpui::div().rounded_b_md()),
        (view! { "div rounded-l-none" }, gpui::div().rounded_l_none()),
        (view! { "div rounded-l-sm" }, gpui::div().rounded_l_sm()),
        (view! { "div rounded-l-md" }, gpui::div().rounded_l_md()),
        (view! { "div rounded-l-lg" }, gpui::div().rounded_l_lg()),
        (view! { "div rounded-l-xl" }, gpui::div().rounded_l_xl()),
        (view! { "div rounded-l-2xl" }, gpui::div().rounded_l_2xl()),
        (view! { "div rounded-l-3xl" }, gpui::div().rounded_l_3xl()),
        (view! { "div rounded-l-full" }, gpui::div().rounded_l_full()),
        (view! { "div rounded-l" }, gpui::div().rounded_l_md()),
        (view! { "div rounded-r-none" }, gpui::div().rounded_r_none()),
        (view! { "div rounded-r-sm" }, gpui::div().rounded_r_sm()),
        (view! { "div rounded-r-md" }, gpui::div().rounded_r_md()),
        (view! { "div rounded-r-lg" }, gpui::div().rounded_r_lg()),
        (view! { "div rounded-r-xl" }, gpui::div().rounded_r_xl()),
        (view! { "div rounded-r-2xl" }, gpui::div().rounded_r_2xl()),
        (view! { "div rounded-r-3xl" }, gpui::div().rounded_r_3xl()),
        (view! { "div rounded-r-full" }, gpui::div().rounded_r_full()),
        (view! { "div rounded-r" }, gpui::div().rounded_r_md()),
        (
            view! { "div rounded-tl-none" },
            gpui::div().rounded_tl_none(),
        ),
        (view! { "div rounded-tl-sm" }, gpui::div().rounded_tl_sm()),
        (view! { "div rounded-tl-md" }, gpui::div().rounded_tl_md()),
        (view! { "div rounded-tl-lg" }, gpui::div().rounded_tl_lg()),
        (view! { "div rounded-tl-xl" }, gpui::div().rounded_tl_xl()),
        (view! { "div rounded-tl-2xl" }, gpui::div().rounded_tl_2xl()),
        (view! { "div rounded-tl-3xl" }, gpui::div().rounded_tl_3xl()),
        (
            view! { "div rounded-tl-full" },
            gpui::div().rounded_tl_full(),
        ),
        (view! { "div rounded-tl" }, gpui::div().rounded_tl_md()),
        (
            view! { "div rounded-tr-none" },
            gpui::div().rounded_tr_none(),
        ),
        (view! { "div rounded-tr-sm" }, gpui::div().rounded_tr_sm()),
        (view! { "div rounded-tr-md" }, gpui::div().rounded_tr_md()),
        (view! { "div rounded-tr-lg" }, gpui::div().rounded_tr_lg()),
        (view! { "div rounded-tr-xl" }, gpui::div().rounded_tr_xl()),
        (view! { "div rounded-tr-2xl" }, gpui::div().rounded_tr_2xl()),
        (view! { "div rounded-tr-3xl" }, gpui::div().rounded_tr_3xl()),
        (
            view! { "div rounded-tr-full" },
            gpui::div().rounded_tr_full(),
        ),
        (view! { "div rounded-tr" }, gpui::div().rounded_tr_md()),
        (
            view! { "div rounded-bl-none" },
            gpui::div().rounded_bl_none(),
        ),
        (view! { "div rounded-bl-sm" }, gpui::div().rounded_bl_sm()),
        (view! { "div rounded-bl-md" }, gpui::div().rounded_bl_md()),
        (view! { "div rounded-bl-lg" }, gpui::div().rounded_bl_lg()),
        (view! { "div rounded-bl-xl" }, gpui::div().rounded_bl_xl()),
        (view! { "div rounded-bl-2xl" }, gpui::div().rounded_bl_2xl()),
        (view! { "div rounded-bl-3xl" }, gpui::div().rounded_bl_3xl()),
        (
            view! { "div rounded-bl-full" },
            gpui::div().rounded_bl_full(),
        ),
        (view! { "div rounded-bl" }, gpui::div().rounded_bl_md()),
        (
            view! { "div rounded-br-none" },
            gpui::div().rounded_br_none(),
        ),
        (view! { "div rounded-br-sm" }, gpui::div().rounded_br_sm()),
        (view! { "div rounded-br-md" }, gpui::div().rounded_br_md()),
        (view! { "div rounded-br-lg" }, gpui::div().rounded_br_lg()),
        (view! { "div rounded-br-xl" }, gpui::div().rounded_br_xl()),
        (view! { "div rounded-br-2xl" }, gpui::div().rounded_br_2xl()),
        (view! { "div rounded-br-3xl" }, gpui::div().rounded_br_3xl()),
        (
            view! { "div rounded-br-full" },
            gpui::div().rounded_br_full(),
        ),
        (view! { "div rounded-br" }, gpui::div().rounded_br_md()),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let _ = view! { "div hover:rounded-tl-full focus:rounded-br-none" };
}

#[test]
fn generated_content_alignment_replaces_prior_wrapped_line_alignment() {
    let cases = [
        (
            view! { "div content-center content-normal" },
            gpui::div().content_normal(),
        ),
        (
            view! { "div content-center content-stretch" },
            gpui::div().content_stretch(),
        ),
        (
            view! { "div content-center content-between" },
            gpui::div().content_between(),
        ),
        (
            view! { "div content-center content-around" },
            gpui::div().content_around(),
        ),
        (
            view! { "div content-center content-evenly" },
            gpui::div().content_evenly(),
        ),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let enabled = true;
    let mut actual = view! { "div content-center class:content-stretch={enabled}" };
    let mut expected = gpui::div().content_stretch();
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    let _ = view! { "div hover:content-between focus:content-around active:content-evenly" };
}

#[test]
fn generated_visible_overflow_resets_only_requested_axes() {
    let cases = [
        (
            view! { "div overflow-hidden overflow-visible" },
            gpui::Overflow::Visible,
            gpui::Overflow::Visible,
        ),
        (
            view! { "div overflow-hidden overflow-x-visible" },
            gpui::Overflow::Visible,
            gpui::Overflow::Hidden,
        ),
        (
            view! { "div overflow-hidden overflow-y-visible" },
            gpui::Overflow::Hidden,
            gpui::Overflow::Visible,
        ),
    ];
    for (mut actual, x, y) in cases {
        assert_eq!(actual.style().overflow.x, Some(x));
        assert_eq!(actual.style().overflow.y, Some(y));
    }
    let enabled = true;
    let _ = view! { "div hover:overflow-visible focus:overflow-x-visible class:overflow-y-visible={enabled}" };
}

#[test]
fn generated_scroll_utilities_compile_without_explicit_ids_and_set_axes() {
    let cases = [
        (view! { "div overflow-auto" }, true, true),
        (view! { "div overflow-scroll" }, true, true),
        (view! { "div overflow-x-auto" }, true, false),
        (view! { "div overflow-x-scroll" }, true, false),
        (view! { "div overflow-y-auto" }, false, true),
        (view! { "div overflow-y-scroll" }, false, true),
    ];
    for (mut actual, x, y) in cases {
        assert_eq!(
            actual.style().overflow.x,
            x.then_some(gpui::Overflow::Scroll)
        );
        assert_eq!(
            actual.style().overflow.y,
            y.then_some(gpui::Overflow::Scroll)
        );
        assert!(gpui::Element::id(&actual).is_some());
    }
    let enabled = true;
    let _ =
        view! { "div hover:overflow-auto focus:overflow-x-scroll class:overflow-y-auto={enabled}" };
}

#[test]
fn dynamic_class_resets_clear_preceding_static_styles_and_preserve_others() {
    let classes = "self-auto aspect-auto";
    let mut actual = view! { "div self-center aspect-square w-full class={classes}" };
    let mut expected = gpui::div().w_full();
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    let classes = "overflow-y-auto";
    let mut actual = view! { "div class={classes}" };
    assert_eq!(actual.style().overflow.y, Some(gpui::Overflow::Scroll));
    assert!(gpui::Element::id(&actual).is_some());
}

#[test]
fn dynamic_state_lists_compile_with_resets_after_static_utilities() {
    let classes = "hover:self-auto focus:aspect-auto active:self-auto";
    let _ = view! { "div hover:self-center focus:aspect-square active:self-end class={classes}" };
}

#[test]
fn generated_shadow_presets_match_runtime_and_clear_previous_shadows() {
    let cases = [
        (
            view! { "div shadow-xl shadow-2xs" },
            gpui::div().shadow_2xs(),
        ),
        (view! { "div shadow-xl shadow-xs" }, gpui::div().shadow_xs()),
        (view! { "div shadow-xl shadow-sm" }, gpui::div().shadow_sm()),
        (view! { "div shadow-xl shadow-md" }, gpui::div().shadow_md()),
        (view! { "div shadow-xl shadow-lg" }, gpui::div().shadow_lg()),
        (view! { "div shadow-xl shadow-xl" }, gpui::div().shadow_xl()),
        (
            view! { "div shadow-xl shadow-2xl" },
            gpui::div().shadow_2xl(),
        ),
        (
            view! { "div shadow-xl shadow-none" },
            gpui::div().shadow_none(),
        ),
        (view! { "div shadow" }, gpui::div().shadow_md()),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let enabled = true;
    let mut actual = view! { "div shadow-lg class:shadow-none={enabled}" };
    let mut expected = gpui::div().shadow_none();
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    let _ = view! { "div hover:shadow-md focus:shadow-lg active:shadow-none" };
}

#[test]
fn child_declarations_are_sequential_and_scoped_to_the_parent() {
    let label = "outer";
    let mut seen = Vec::new();
    let _ = view! {r#"div
  span class={{ seen.push(label); "p-2" }}
  $: let label = "first"
  span class={{ seen.push(label); "p-2" }}
  $: let label = "second"
  span class={{ seen.push(label); "p-2" }}
"#};
    assert_eq!(seen, ["outer", "first", "second"]);
    assert_eq!(label, "outer");
}

#[test]
fn child_declarations_work_in_conditionals_and_loop_bodies() {
    let enabled = true;
    let items = [1, 2];
    let mut seen = Vec::new();
    let _ = view! {r#"div
  if enabled
    $: let value = 7
    span class={{ seen.push(value); "p-2" }}
  for item in items.into_iter()
    $: let value = item
    span class={{ seen.push(value); "p-2" }}
"#};
    assert_eq!(seen, [7, 1, 2]);
}

#[test]
fn declaration_only_branches_compile_and_conditions_run_once() {
    let mut checks = 0;
    let mut seen = Vec::new();
    let mut record = |value| {
        seen.push(value);
        value
    };
    let mut condition = || {
        checks += 1;
        true
    };
    let _ = view! {r#"div
  if condition()
    $: let _value = record(1)
  else
    $: let _value = record(2)
  for item in [3, 4].into_iter()
    $: let _value = record(item)
"#};
    assert_eq!(checks, 1);
    assert_eq!(seen, [1, 3, 4]);
}

#[test]
fn root_declarations_preserve_sibling_order_and_single_root_shape() {
    let label = "outer";
    let mut seen = Vec::new();
    let _ = view! {r#"span class={{ seen.push(label); "p-2" }}
$: let label = "later"
span class={{ seen.push(label); "p-2" }}
"#};
    assert_eq!(seen, ["outer", "later"]);
    assert_eq!(label, "outer");
    let mut actual = view! {r#"div p-2
$: let _unused = 1
"#};
    let mut expected = gpui::div().p_2();
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn match_declarations_execute_only_the_selected_arm_once() {
    let mut checks = 0;
    let mut seen = Vec::new();
    let mut selector = || {
        checks += 1;
        2
    };
    let mut record = |value| {
        seen.push(value);
        value
    };
    let _ = view! {r#"div
  match selector()
    1 =>
      $: let _value = record(1)
    2 =>
      $: let value = record(2)
      span class={{ record(value + 1); "p-2" }}
    _ =>
      $: let _value = record(4)
"#};
    assert_eq!(checks, 1);
    assert_eq!(seen, [2, 3]);
    let mut record = |value| {
        seen.push(value);
        value
    };
    let _ = view! {r#"match 1
  1 =>
    $: let _value = record(5)
  _ =>
    $: let _value = record(6)
"#};
    assert_eq!(seen, [2, 3, 5]);
}

#[test]
fn declaration_scope_builder_names_do_not_shadow_caller_variables() {
    let __crepus_parent = "parent";
    let __crepus_root = "root";
    let mut seen = Vec::new();
    let _ = view! {r#"div
  $: let _value = 1
  span class={{ seen.push(__crepus_parent); "p-2" }}
"#};
    let mut record = |value| {
        seen.push(value);
        value
    };
    let _ = view! {r#"div
$: let _value = record(__crepus_root)
"#};
    assert_eq!(seen, ["parent", "root"]);
}

#[test]
fn control_flow_builders_do_not_capture_caller_el_variables() {
    let __el = true;
    let mut seen = Vec::new();
    let _ = view! {r#"div
  if __el
    span class={{ seen.push(__el); "p-2" }}
    span
  else
    span
"#};
    assert_eq!(seen, [true]);
    let __el = 0xff0000u32;
    let enabled = true;
    let mut actual = view! { "div class:bg-{__el}={enabled}" };
    let mut expected = gpui::div().bg(gpui::rgb(__el));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn state_style_builders_do_not_shadow_caller_s_variables() {
    let __s = 0xff0000u32;
    let _ = view! { "div hover:bg-{__s} focus:bg-{__s} active:text-{__s}" };
    let classes = "hover:p-2";
    let _ = view! { "div hover:bg-{__s} class={classes} hover:text-{__s}" };
}

#[test]
fn conditional_state_classes_merge_with_static_states_and_evaluate_once() {
    let mut checks = 0;
    let mut condition = || {
        checks += 1;
        true
    };
    let _ = view! { "div hover:p-2 class:hover:bg-red-500={condition()} class:hover:text-white={false} class:focus:p-4={true} class:active:bg-blue-500={true}" };
    assert_eq!(checks, 1);
}

#[test]
fn generated_focus_styles_construct_stateful_elements_for_keyboard_focus() {
    let actual = view! { "div focus:bg-red-500" };
    assert!(gpui::Element::id(&actual).is_some());
    let enabled = true;
    let actual = view! { "div class:focus:p-2={enabled}" };
    assert!(gpui::Element::id(&actual).is_some());
    let classes = "focus:bg-blue-500";
    let actual = view! { "div class={classes}" };
    assert!(gpui::Element::id(&actual).is_some());
    let classes = "p-2";
    let _ = view! { "div class={classes}" };
}

#[gpui::test]
fn generated_focus_styles_navigate_and_skip_disabled_conditions(cx: &mut gpui::TestAppContext) {
    struct FocusView;
    impl gpui::Render for FocusView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let disabled = false;
            let classes = "focus:bg-blue-500";
            view! {r#"div flex flex-col
  button w-[100px] h-[40px] focus:bg-red-500
  button w-[100px] h-[40px] class:focus:bg-green-500={disabled}
  button w-[100px] h-[40px] class={classes}
"#}
        }
    }
    let (_, cx) = cx.add_window_view(|_, _| FocusView);
    cx.simulate_click(
        gpui::point(gpui::px(10.0), gpui::px(10.0)),
        gpui::Modifiers::default(),
    );
    let first = cx.update(|window, app| window.focused(app).unwrap());
    cx.update(|window, app| window.focus_next(app));
    let next = cx.update(|window, app| window.focused(app).unwrap());
    assert_ne!(first, next);
    cx.simulate_click(
        gpui::point(gpui::px(10.0), gpui::px(90.0)),
        gpui::Modifiers::default(),
    );
    assert_eq!(cx.update(|window, app| window.focused(app).unwrap()), next);
    cx.update(|window, app| window.focus_prev(app));
    assert_eq!(cx.update(|window, app| window.focused(app).unwrap()), first);
}

#[cfg(target_os = "macos")]
static METAL_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal device"]
fn generated_focus_colors_paint_and_skip_disabled_controls() {
    let _guard = METAL_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    use gpui::{AppContext, InputEvent};
    struct FocusPaintView;
    impl gpui::Render for FocusPaintView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let enabled = true;
            let disabled = false;
            let classes = "focus:bg-[#0000ff]";
            view! {r#"div flex flex-col bg-white
  button w-[100px] h-[20px] focus:bg-[#ff0000]
  button w-[100px] h-[20px] class:focus:bg-[#00ff00]={enabled}
  button w-[100px] h-[20px] class:focus:bg-black={disabled}
  button w-[100px] h-[20px] class={classes}
"#}
        }
    }
    let platform = gpui_platform::current_platform(true);
    let mut cx = gpui::HeadlessAppContext::with_platform(
        platform.text_system(),
        std::sync::Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let window = cx
        .open_window(gpui::size(gpui::px(120.0), gpui::px(100.0)), |_, app| {
            app.new(|_| FocusPaintView)
        })
        .unwrap()
        .into();
    let pixels = |cx: &mut gpui::HeadlessAppContext| {
        cx.update_window(window, |_, window, app| window.draw(app).clear(app))
            .unwrap();
        let image = cx.capture_screenshot(window).unwrap();
        let scale = image.width() / 120;
        [10, 30, 50, 70].map(|y| image.get_pixel(10 * scale, y * scale).0)
    };
    let white = [255, 255, 255, 255];
    assert_eq!(pixels(&mut cx), [white; 4]);
    cx.update_window(window, |_, window, app| {
        window.dispatch_event(
            gpui::MouseDownEvent {
                position: gpui::point(gpui::px(10.0), gpui::px(10.0)),
                button: gpui::MouseButton::Left,
                ..Default::default()
            }
            .to_platform_input(),
            app,
        );
    })
    .unwrap();
    assert_eq!(pixels(&mut cx), [[255, 0, 0, 255], white, white, white]);
    cx.update_window(window, |_, window, app| window.focus_next(app))
        .unwrap();
    assert_eq!(pixels(&mut cx), [white, [0, 255, 0, 255], white, white]);
    cx.update_window(window, |_, window, app| window.focus_next(app))
        .unwrap();
    assert_eq!(pixels(&mut cx), [white, white, white, [0, 0, 255, 255]]);
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal device"]
fn generated_static_and_dynamic_scroll_containers_move_independently() {
    let _guard = METAL_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    use gpui::{AppContext, InputEvent};
    struct ScrollPaintView;
    impl gpui::Render for ScrollPaintView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let classes = "overflow-x-auto";
            view! {r#"div flex flex-row gap-[10px] bg-white
  div flex flex-col w-[40px] h-[40px] overflow-y-auto
    div shrink-0 w-[40px] h-[40px] bg-[#ff0000]
    div shrink-0 w-[40px] h-[40px] bg-[#0000ff]
  div flex flex-row w-[40px] h-[40px] class={classes}
    div shrink-0 w-[40px] h-[40px] bg-[#ff0000]
    div shrink-0 w-[40px] h-[40px] bg-[#0000ff]
"#}
        }
    }
    let platform = gpui_platform::current_platform(true);
    let mut cx = gpui::HeadlessAppContext::with_platform(
        platform.text_system(),
        std::sync::Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let window = cx
        .open_window(gpui::size(gpui::px(120.0), gpui::px(100.0)), |_, app| {
            app.new(|_| ScrollPaintView)
        })
        .unwrap()
        .into();
    let pixels = |cx: &mut gpui::HeadlessAppContext| {
        cx.update_window(window, |_, window, app| window.draw(app).clear(app))
            .unwrap();
        let image = cx.capture_screenshot(window).unwrap();
        let scale = image.width() / 120;
        [10, 60].map(|x| image.get_pixel(x * scale, 10 * scale).0)
    };
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    assert_eq!(pixels(&mut cx), [red, red]);
    for (x, dx, dy, expected) in [
        (10.0, 0.0, -40.0, [blue, red]),
        (60.0, -40.0, 0.0, [blue, blue]),
    ] {
        cx.update_window(window, |_, window, app| {
            window.dispatch_event(
                gpui::ScrollWheelEvent {
                    position: gpui::point(gpui::px(x), gpui::px(10.0)),
                    delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(dx), gpui::px(dy))),
                    ..Default::default()
                }
                .to_platform_input(),
                app,
            );
        })
        .unwrap();
        assert_eq!(pixels(&mut cx), expected);
        assert_eq!(pixels(&mut cx), expected);
    }
}

#[gpui::test]
fn generated_loop_focus_identities_are_distinct_across_iterations(cx: &mut gpui::TestAppContext) {
    struct LoopFocusView;
    impl gpui::Render for LoopFocusView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            view! {r#"div flex flex-col
  for _outer in [1, 2].into_iter()
    for _inner in [1, 2].into_iter()
      button w-[100px] h-[20px] focus:bg-red-500
"#}
        }
    }
    let (_, cx) = cx.add_window_view(|_, _| LoopFocusView);
    let mut handles = Vec::new();
    for y in [10.0, 30.0, 50.0, 70.0] {
        cx.simulate_click(
            gpui::point(gpui::px(10.0), gpui::px(y)),
            gpui::Modifiers::default(),
        );
        let handle = cx.update(|window, app| window.focused(app).unwrap());
        assert!(!handles.contains(&handle));
        handles.push(handle);
    }
    cx.update(|window, app| window.focus_prev(app));
    assert_eq!(
        cx.update(|window, app| window.focused(app).unwrap()),
        handles[2]
    );
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal device"]
fn generated_loop_scroll_containers_move_independently() {
    let _guard = METAL_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    use gpui::{AppContext, InputEvent};
    struct ScrollPaintView;
    impl gpui::Render for ScrollPaintView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let items = ["flex-col overflow-y-auto", "flex-row overflow-x-auto"];
            view! {r#"div flex flex-row gap-[10px] bg-white
  for classes in items.into_iter()
    div w-[40px] h-[40px] flex class={classes}
      div shrink-0 w-[40px] h-[40px] bg-[#ff0000]
      div shrink-0 w-[40px] h-[40px] bg-[#0000ff]
"#}
        }
    }
    let platform = gpui_platform::current_platform(true);
    let mut cx = gpui::HeadlessAppContext::with_platform(
        platform.text_system(),
        std::sync::Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let window = cx
        .open_window(gpui::size(gpui::px(120.0), gpui::px(100.0)), |_, app| {
            app.new(|_| ScrollPaintView)
        })
        .unwrap()
        .into();
    let pixels = |cx: &mut gpui::HeadlessAppContext| {
        cx.update_window(window, |_, window, app| window.draw(app).clear(app))
            .unwrap();
        let image = cx.capture_screenshot(window).unwrap();
        let scale = image.width() / 120;
        [10, 60].map(|x| image.get_pixel(x * scale, 10 * scale).0)
    };
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    assert_eq!(pixels(&mut cx), [red, red]);
    for (x, dx, dy, expected) in [
        (10.0, 0.0, -40.0, [blue, red]),
        (60.0, -40.0, 0.0, [blue, blue]),
    ] {
        cx.update_window(window, |_, window, app| {
            window.dispatch_event(
                gpui::ScrollWheelEvent {
                    position: gpui::point(gpui::px(x), gpui::px(10.0)),
                    delta: gpui::ScrollDelta::Pixels(gpui::point(gpui::px(dx), gpui::px(dy))),
                    ..Default::default()
                }
                .to_platform_input(),
                app,
            );
        })
        .unwrap();
        assert_eq!(pixels(&mut cx), expected);
        assert_eq!(pixels(&mut cx), expected);
    }
}

#[test]
fn unsupported_gpui_utilities_do_not_generate_nonexistent_methods() {
    let mut actual = view! { "div relative p-2 fixed sticky select-none pointer-events-none hover:fixed focus:sticky" };
    let mut expected = gpui::div().relative().p_2();
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn generated_text_decorations_match_native_styles_and_resets() {
    let cases = [
        (
            view! { "div underline line-through underline" },
            gpui::div().underline().line_through().underline(),
        ),
        (
            view! { "div underline line-through line-through" },
            gpui::div().underline().line_through().line_through(),
        ),
        (
            view! { "div underline line-through no-underline" },
            gpui::div()
                .underline()
                .line_through()
                .text_decoration_none(),
        ),
        (
            view! { "div underline line-through decoration-solid" },
            gpui::div()
                .underline()
                .line_through()
                .text_decoration_solid(),
        ),
        (
            view! { "div underline line-through decoration-wavy" },
            gpui::div()
                .underline()
                .line_through()
                .text_decoration_wavy(),
        ),
        (
            view! { "div underline line-through decoration-0" },
            gpui::div().underline().line_through().text_decoration_0(),
        ),
        (
            view! { "div underline line-through decoration-1" },
            gpui::div().underline().line_through().text_decoration_1(),
        ),
        (
            view! { "div underline line-through decoration-2" },
            gpui::div().underline().line_through().text_decoration_2(),
        ),
        (
            view! { "div underline line-through decoration-4" },
            gpui::div().underline().line_through().text_decoration_4(),
        ),
        (
            view! { "div underline line-through decoration-8" },
            gpui::div().underline().line_through().text_decoration_8(),
        ),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let enabled = true;
    let _ = view! { "div hover:underline focus:decoration-wavy active:no-underline class:line-through={enabled}" };
}

#[test]
fn generated_text_overflow_and_whitespace_match_supported_gpui_styles() {
    let cases = [
        (view! { "div text-ellipsis" }, gpui::div().text_ellipsis()),
        (
            view! { "div whitespace-nowrap whitespace-normal whitespace-pre" },
            gpui::div().whitespace_normal(),
        ),
        (
            view! { "div truncate whitespace-normal" },
            gpui::div().truncate().whitespace_normal(),
        ),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let enabled = true;
    let _ =
        view! { "div hover:text-ellipsis focus:whitespace-normal class:text-ellipsis={enabled}" };
}

#[test]
fn generated_color_opacity_suffixes_match_runtime_alpha_policy() {
    let mut actual = view! { "div bg-white/50.5 bg-white/NaN text-[#1234]/25 text-[#12]/50 border-transparent/50" };
    let mut expected = gpui::div()
        .bg(gpui::rgba(0xffffff80))
        .text_color(gpui::rgba(0x1122333f))
        .border_color(gpui::rgba(0));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    let mut actual = view! { "div bg-red-500/50 border-[#aabbcc80]/100" };
    let mut expected = gpui::div()
        .bg(gpui::rgba(0xfb2c367f))
        .border_color(gpui::rgba(0xaabbccff));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    let enabled = true;
    let _ = view! { "div hover:bg-white/50 focus:text-[#123]/75 class:border-white/50={enabled}" };
}

#[test]
fn bound_color_opacity_suffixes_evaluate_once_and_preserve_invalid_overrides() {
    let mut reads = 0;
    let mut color = || {
        reads += 1;
        0x123456u32
    };
    let mut actual = view! { "div bg-{color()}/50 bg-{color()}/NaN" };
    let mut expected = gpui::div().bg(gpui::rgba(0x1234567f));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    assert_eq!(reads, 1);
    let color = 0xaabbccu32;
    let mut actual = view! { "div text-{color}/25 border-{color}/100" };
    let mut expected = gpui::div()
        .text_color(gpui::rgba(0xaabbcc3f))
        .border_color(gpui::rgba(0xaabbccff));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    let color = 0x224466u32;
    let mut actual = view! { "div bg-{color/2}" };
    let mut expected = gpui::div().bg(gpui::rgb(0x112233));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn malformed_bound_color_suffixes_do_not_execute_the_expression() {
    let reads = std::cell::Cell::new(0);
    let color = || {
        reads.set(reads.get() + 1);
        0x123456u32
    };
    let mut actual = view! { "div bg-white bg-{color()}junk text-{color()}junk/50" };
    let mut expected = gpui::div().bg(gpui::white());
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    assert_eq!(reads.get(), 0);
    let _ = color();
}

#[test]
fn bound_color_expressions_preserve_quoted_braces_through_template_parsing() {
    let color = |value: &str| match value {
        "}" => 0x123456u32,
        "{" => 0xaabbccu32,
        _ => 0u32,
    };
    let mut actual = view! {r#"div bg-{color("}")} text-{color("{")}/50"#};
    let mut expected = gpui::div()
        .bg(gpui::rgb(0x123456))
        .text_color(gpui::rgba(0xaabbcc7f));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal device"]
fn generated_color_opacity_blends_static_and_bound_colors() {
    let _guard = METAL_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    use gpui::AppContext;
    struct OpacityPaintView;
    impl gpui::Render for OpacityPaintView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let color = 0x0000ffu32;
            view! {r#"div flex flex-row bg-white
  div w-[40px] h-[40px] bg-[#f00]/50
  div w-[40px] h-[40px] bg-{color}/50
  div w-[40px] h-[40px] bg-transparent/100
"#}
        }
    }
    let platform = gpui_platform::current_platform(true);
    let mut cx = gpui::HeadlessAppContext::with_platform(
        platform.text_system(),
        std::sync::Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let window = cx
        .open_window(gpui::size(gpui::px(120.0), gpui::px(60.0)), |_, app| {
            app.new(|_| OpacityPaintView)
        })
        .unwrap()
        .into();
    cx.update_window(window, |_, window, app| window.draw(app).clear(app))
        .unwrap();
    let image = cx.capture_screenshot(window).unwrap();
    let scale = image.width() / 120;
    let actual = [10, 50, 90].map(|x| image.get_pixel(x * scale, 10 * scale).0);
    assert_eq!(
        actual,
        [
            [255, 128, 128, 255],
            [128, 128, 255, 255],
            [255, 255, 255, 255]
        ]
    );
}

#[test]
fn generated_named_and_bracketed_colors_share_runtime_resolution() {
    let mut actual = view! { "div bg-[white] text-[red-500] border-white border-transparent" };
    let mut expected = gpui::div()
        .bg(gpui::white())
        .text_color(gpui::rgb(0xfb2c36))
        .border_color(gpui::rgba(0));
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
    let mut actual = view! { "div bg-clear text-white text-clear border-[black]" };
    let mut expected = gpui::div()
        .bg(gpui::rgba(0))
        .text_color(gpui::rgba(0))
        .border_color(gpui::black());
    assert_eq!(
        format!("{:?}", actual.style()),
        format!("{:?}", expected.style())
    );
}

#[test]
fn generated_cursor_utilities_match_runtime_and_override_pointer() {
    let cases = [
        (
            view! { "div cursor-pointer cursor-move" },
            gpui::div().cursor_move(),
        ),
        (
            view! { "div cursor-pointer cursor-context-menu" },
            gpui::div().cursor_context_menu(),
        ),
        (
            view! { "div cursor-pointer cursor-vertical-text" },
            gpui::div().cursor_vertical_text(),
        ),
        (
            view! { "div cursor-pointer cursor-alias" },
            gpui::div().cursor_alias(),
        ),
        (
            view! { "div cursor-pointer cursor-copy" },
            gpui::div().cursor_copy(),
        ),
        (
            view! { "div cursor-pointer cursor-no-drop" },
            gpui::div().cursor_no_drop(),
        ),
        (
            view! { "div cursor-pointer cursor-ew-resize" },
            gpui::div().cursor_ew_resize(),
        ),
        (
            view! { "div cursor-pointer cursor-ns-resize" },
            gpui::div().cursor_ns_resize(),
        ),
        (
            view! { "div cursor-pointer cursor-nesw-resize" },
            gpui::div().cursor_nesw_resize(),
        ),
        (
            view! { "div cursor-pointer cursor-nwse-resize" },
            gpui::div().cursor_nwse_resize(),
        ),
        (
            view! { "div cursor-pointer cursor-n-resize" },
            gpui::div().cursor_n_resize(),
        ),
        (
            view! { "div cursor-pointer cursor-e-resize" },
            gpui::div().cursor_e_resize(),
        ),
        (
            view! { "div cursor-pointer cursor-s-resize" },
            gpui::div().cursor_s_resize(),
        ),
        (
            view! { "div cursor-pointer cursor-w-resize" },
            gpui::div().cursor_w_resize(),
        ),
    ];
    for (mut actual, mut expected) in cases {
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let enabled = true;
    let _ = view! { "div hover:cursor-copy focus:cursor-move class:cursor-ew-resize={enabled}" };
}

#[test]
fn generated_ring_widths_match_runtime_and_ignore_invalid_overrides() {
    let cases = [
        (view! { "div shadow-xl ring" }, "ring"),
        (view! { "div shadow-xl ring-0" }, "ring-0"),
        (view! { "div shadow-xl ring-1" }, "ring-1"),
        (view! { "div shadow-xl ring-2" }, "ring-2"),
        (view! { "div shadow-xl ring-4" }, "ring-4"),
        (view! { "div shadow-xl ring-8" }, "ring-8"),
        (view! { "div shadow-xl ring-[2px]" }, "ring-[2px]"),
        (view! { "div shadow-xl ring-[0.5rem]" }, "ring-[0.5rem]"),
        (
            view! { "div shadow-xl ring-2 ring-[1e38rem] ring-[50%] ring-[NaNpx]" },
            "ring-2",
        ),
    ];
    for (mut actual, class) in cases {
        let mut expected = crepuscularity_gpui::crepuscularity_runtime::styler::apply_class(
            gpui::div().shadow_xl(),
            class,
        );
        assert_eq!(
            format!("{:?}", actual.style()),
            format!("{:?}", expected.style())
        );
    }
    let enabled = true;
    let _ = view! { "div hover:ring-2 focus:ring-[0.5rem] class:ring-0={enabled}" };
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal device"]
fn generated_inset_ring_paints_inside_the_box() {
    let _guard = METAL_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    use gpui::AppContext;
    struct InsetPaintView;
    impl gpui::Render for InsetPaintView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            view! { "div size-[60px] bg-white\n  div m-[8px] size-[40px] bg-white ring-inset ring-4" }
        }
    }
    let platform = gpui_platform::current_platform(true);
    let mut cx = gpui::HeadlessAppContext::with_platform(
        platform.text_system(),
        std::sync::Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let window = cx
        .open_window(gpui::size(gpui::px(60.0), gpui::px(60.0)), |_, app| {
            app.new(|_| InsetPaintView)
        })
        .unwrap()
        .into();
    cx.update_window(window, |_, window, app| window.draw(app).clear(app))
        .unwrap();
    let image = cx.capture_screenshot(window).unwrap();
    let scale = image.width() / 60;
    let actual = [6, 9, 20].map(|x| image.get_pixel(x * scale, 20 * scale).0);
    assert_eq!(actual[0], [255, 255, 255, 255]);
    assert_ne!(actual[1], [255, 255, 255, 255]);
    assert_eq!(actual[1][3], 255);
    assert_eq!(actual[2], [255, 255, 255, 255]);
}

#[test]
fn generated_inset_rings_preserve_ordinary_shadows_in_either_order() {
    let mut first = view! { "div shadow-md ring-inset ring-2" };
    let mut second = view! { "div ring-2 shadow-md ring-inset" };
    assert_eq!(
        format!("{:?}", first.style()),
        format!("{:?}", second.style())
    );
    let shadows = first.style().box_shadow.as_ref().unwrap();
    assert!(shadows.iter().any(|shadow| shadow.inset));
    assert!(shadows.iter().any(|shadow| !shadow.inset));
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal device"]
fn composed_inset_rings_and_shadows_paint_equally_in_both_orders() {
    let _guard = METAL_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    use gpui::AppContext;
    struct ComposedPaintView;
    impl gpui::Render for ComposedPaintView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            view! {r#"div flex flex-row gap-[20px] bg-white
  div m-[8px] size-[40px] bg-white shadow-lg ring-inset ring-4
  div m-[8px] size-[40px] bg-white ring-4 ring-inset shadow-lg
"#}
        }
    }
    let platform = gpui_platform::current_platform(true);
    let mut cx = gpui::HeadlessAppContext::with_platform(
        platform.text_system(),
        std::sync::Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let window = cx
        .open_window(gpui::size(gpui::px(160.0), gpui::px(80.0)), |_, app| {
            app.new(|_| ComposedPaintView)
        })
        .unwrap()
        .into();
    cx.update_window(window, |_, window, app| window.draw(app).clear(app))
        .unwrap();
    let image = cx.capture_screenshot(window).unwrap();
    let scale = image.width() / 160;
    let sample =
        |offset: u32| [6, 9, 28].map(|x| image.get_pixel((x + offset) * scale, 28 * scale).0);
    let first = sample(0);
    assert_eq!(first, sample(76));
    assert_ne!(
        first[0],
        [255, 255, 255, 255],
        "ordinary shadow remains outside"
    );
    assert_eq!(first[0][0], first[0][1]);
    assert_eq!(first[0][1], first[0][2]);
    assert_ne!(
        first[1][0], first[1][2],
        "colored inset ring remains inside"
    );
    assert_eq!(first[2], [255, 255, 255, 255], "box interior remains white");
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal device"]
fn generated_state_shadow_resets_preserve_the_other_shadow_kind() {
    check_generated_state_shadow_resets("hover:ring-0", "hover:shadow-none");
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal device"]
fn generated_target_prefixed_state_shadow_resets_preserve_the_other_shadow_kind() {
    check_generated_state_shadow_resets("gpui:hover:gui:gpui:ring-0", "hover:gui:shadow-none");
}

#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires a Metal device"]
fn generated_static_target_prefixed_state_shadow_resets_preserve_the_other_shadow_kind() {
    check_generated_state_shadow_resets("static-prefixed", "hover:gui:shadow-none");
}

#[cfg(target_os = "macos")]
fn check_generated_state_shadow_resets(first: &'static str, second: &'static str) {
    let _guard = METAL_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    use gpui::{AppContext, InputEvent};
    struct StateShadowView(&'static str, &'static str);
    impl gpui::Render for StateShadowView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let first = self.0;
            let classes = self.1;
            if first == "static-prefixed" {
                let enabled = true;
                return view! {r#"div flex flex-row gap-[20px] bg-white
  div m-[8px] size-[40px] bg-white shadow-lg ring-inset ring-4 hover:gpui:gui:ring-0
  div m-[8px] size-[40px] bg-white shadow-lg ring-inset ring-4 class:hover:gui:shadow-none={enabled}
"#};
            }
            if first == "hover:ring-0" {
                return view! {r#"div flex flex-row gap-[20px] bg-white
  div m-[8px] size-[40px] bg-white shadow-lg ring-inset ring-4 hover:ring-0
  div m-[8px] size-[40px] bg-white shadow-lg ring-inset ring-4 class={classes}
"#};
            }
            view! {r#"div flex flex-row gap-[20px] bg-white
  div m-[8px] size-[40px] bg-white shadow-lg ring-inset ring-4 class={first}
  div m-[8px] size-[40px] bg-white shadow-lg ring-inset ring-4 class={classes}
"#}
        }
    }
    let platform = gpui_platform::current_platform(true);
    let mut cx = gpui::HeadlessAppContext::with_platform(
        platform.text_system(),
        std::sync::Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let window = cx
        .open_window(gpui::size(gpui::px(160.0), gpui::px(80.0)), |_, app| {
            app.new(|_| StateShadowView(first, second))
        })
        .unwrap()
        .into();
    let pixels = |cx: &mut gpui::HeadlessAppContext| {
        cx.update_window(window, |_, window, app| window.draw(app).clear(app))
            .unwrap();
        let image = cx.capture_screenshot(window).unwrap();
        let scale = image.width() / 160;
        [6, 9, 82, 85].map(|x| image.get_pixel(x * scale, 28 * scale).0)
    };
    let initial = pixels(&mut cx);
    for x in [28.0, 104.0] {
        cx.update_window(window, |_, window, app| {
            window.dispatch_event(
                gpui::MouseMoveEvent {
                    position: gpui::point(gpui::px(x), gpui::px(28.0)),
                    ..Default::default()
                }
                .to_platform_input(),
                app,
            );
        })
        .unwrap();
        let actual = pixels(&mut cx);
        if x == 28.0 {
            assert_eq!(actual[0], initial[0]);
            assert_eq!(actual[1], [255, 255, 255, 255]);
        } else {
            assert_eq!(actual[1], initial[1]);
            assert_eq!(actual[2], [255, 255, 255, 255]);
            assert_eq!(actual[3], initial[3]);
        }
    }
}
