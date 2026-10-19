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
