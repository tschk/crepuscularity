#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use serde::Deserialize;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

const API_BASE: &str = "https://crates.io";
const SEARCH_URL: &str = "https://crates.io/api/v1/crates?page=1&per_page=100&q=crepuscularity";
const SVG_WIDTH: u32 = 720;
const SVG_HEIGHT: u32 = 220;
const SVG_PAD: u32 = 18;
const UPDATE_INTERVAL_MS: i32 = 60_000;
const INITIAL_RETRY_MS: i32 = 100;
const INITIAL_RETRY_ATTEMPTS: u32 = 100;

#[wasm_bindgen]
pub fn crepus_render(bundle_json: &str) -> Result<String, JsValue> {
    crepuscularity_web::render_bundle(bundle_json).map_err(|e| JsValue::from_str(&e.to_string()))
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start_downloads_widget() {
    schedule_initial_downloads_refresh(0, INITIAL_RETRY_ATTEMPTS);
    schedule_downloads_interval();
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct CrateInfo {
    name: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    recent_downloads: u64,
    #[serde(default)]
    newest_version: Option<String>,
    #[serde(default)]
    max_version: Option<String>,
    #[serde(default)]
    default_version: Option<String>,
    links: CrateLinks,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct CrateLinks {
    version_downloads: String,
}

#[derive(Clone, Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    crates: Vec<CrateInfo>,
}

#[derive(Clone, Debug, Deserialize)]
struct DownloadsResponse {
    #[serde(default)]
    version_downloads: Vec<DownloadRow>,
    #[serde(default)]
    meta: DownloadsMeta,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct DownloadsMeta {
    #[serde(default)]
    extra_downloads: Vec<DownloadRow>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct DownloadRow {
    date: String,
    #[serde(default)]
    downloads: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DailyDownloads {
    date: String,
    downloads: u64,
}

struct DownloadsData {
    series: Vec<DailyDownloads>,
    crates: Vec<CrateDownloads>,
}

struct CrateDownloads {
    info: CrateInfo,
    series: Vec<DailyDownloads>,
}

fn filter_crepuscularity_crates(mut crates: Vec<CrateInfo>) -> Vec<CrateInfo> {
    crates.retain(|krate| krate.name.starts_with("crepuscularity"));
    crates.sort_by(|left, right| {
        right
            .downloads
            .cmp(&left.downloads)
            .then_with(|| left.name.cmp(&right.name))
    });
    crates
}

fn aggregate_daily_downloads(payloads: &[DownloadsResponse]) -> Vec<DailyDownloads> {
    let mut daily = BTreeMap::new();

    for payload in payloads {
        for row in payload
            .version_downloads
            .iter()
            .chain(payload.meta.extra_downloads.iter())
        {
            if row.date.is_empty() {
                continue;
            }
            *daily.entry(row.date.clone()).or_insert(0) += row.downloads;
        }
    }

    daily
        .into_iter()
        .map(|(date, downloads)| DailyDownloads { date, downloads })
        .collect()
}

fn cumulative_downloads(series: impl IntoIterator<Item = DailyDownloads>) -> Vec<DailyDownloads> {
    let mut running_total = 0;
    series
        .into_iter()
        .map(|mut row| {
            running_total += row.downloads;
            row.downloads = running_total;
            row
        })
        .collect()
}

fn overall_cumulative_downloads(
    series: impl IntoIterator<Item = DailyDownloads>,
    total: u64,
) -> Vec<DailyDownloads> {
    let mut cumulative: Vec<_> = series.into_iter().collect();
    let series_total = cumulative.iter().map(|row| row.downloads).sum::<u64>();
    let effective_total = total.max(series_total);
    let mut running_total = effective_total.saturating_sub(series_total);

    for row in &mut cumulative {
        running_total = running_total.saturating_add(row.downloads);
        row.downloads = running_total;
    }

    if let Some(last) = cumulative.last_mut() {
        last.downloads = effective_total;
    }

    cumulative
}

fn cumulative_downloads_at_dates(
    series: &[DailyDownloads],
    total: u64,
    dates: &[String],
) -> Vec<DailyDownloads> {
    let series_total = series.iter().map(|row| row.downloads).sum::<u64>();
    let mut running_total = total.saturating_sub(series_total);
    let mut daily = BTreeMap::new();

    for row in series {
        daily.insert(row.date.as_str(), row.downloads);
    }

    dates
        .iter()
        .map(|date| {
            running_total = running_total.saturating_add(*daily.get(date.as_str()).unwrap_or(&0));
            DailyDownloads {
                date: date.clone(),
                downloads: running_total,
            }
        })
        .collect()
}

fn points_for_series(series: &[DailyDownloads], width: u32, height: u32, pad: u32) -> Vec<(u32, u32)> {
    if series.is_empty() {
        return Vec::new();
    }

    let min_downloads = series.iter().map(|row| row.downloads).min().unwrap_or(0);
    let max_downloads = series.iter().map(|row| row.downloads).max().unwrap_or(1);
    let download_span = max_downloads.saturating_sub(min_downloads).max(1) as f64;
    let x_span = series.len().saturating_sub(1).max(1) as f64;
    let inner_width = width.saturating_sub(pad * 2) as f64;
    let inner_height = height.saturating_sub(pad * 2) as f64;

    series
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let x = pad as f64 + (index as f64 / x_span) * inner_width;
            let y = height as f64
                - pad as f64
                - ((row.downloads.saturating_sub(min_downloads) as f64) / download_span)
                    * inner_height;
            (x.round() as u32, y.round() as u32)
        })
        .collect()
}

fn format_exact_number(value: u64) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    let first_group_len = digits.len() % 3;

    for (index, ch) in digits.chars().enumerate() {
        if index > 0 {
            let split = index == first_group_len
                || (index > first_group_len && (index - first_group_len) % 3 == 0);
            if split {
                formatted.push(' ');
            }
        }
        formatted.push(ch);
    }

    formatted
}

fn render_slot_number(value: u64) -> String {
    format_slot_transition(None, &format_exact_number(value))
}

fn render_static_slot_number(value: u64) -> String {
    format_static_slot_number(&format_exact_number(value))
}

fn format_static_slot_number(value: &str) -> String {
    let mut html = String::from(r#"<span class="downloads-number-slot">"#);

    for ch in value.chars() {
        html.push_str(&format!(
            r#"<span class="downloads-slot-char">{}</span>"#,
            escape_html(&ch.to_string())
        ));
    }

    html.push_str("</span>");
    html
}

fn format_slot_transition(previous: Option<&str>, value: &str) -> String {
    let old = previous.unwrap_or("");
    let animate_all = old.chars().count() != value.chars().count();
    let old_chars = old.chars().collect::<Vec<_>>();
    let new_chars = value.chars().collect::<Vec<_>>();
    let mut html = String::from(r#"<span class="downloads-number-slot">"#);

    for (index, ch) in new_chars.iter().enumerate() {
        let changed = animate_all || old_chars.get(index).is_none_or(|old| old != ch);
        let class_name = if changed {
            "downloads-slot-char downloads-slot-changed"
        } else {
            "downloads-slot-char"
        };
        html.push_str(&format!(
            r#"<span class="{class_name}">{}</span>"#,
            escape_html(&ch.to_string())
        ));
    }

    html.push_str("</span>");
    html
}

fn format_duration_ms(value: u64) -> String {
    if value < 1_000 {
        format!("{value} ms")
    } else {
        let seconds = value as f64 / 1_000.0;
        format!("{seconds:.1} s")
    }
}

fn format_date(value: &str) -> String {
    if value.len() != 10 {
        return value.to_string();
    }

    let year = &value[0..4];
    let month = match &value[5..7] {
        "01" => "Jan",
        "02" => "Feb",
        "03" => "Mar",
        "04" => "Apr",
        "05" => "May",
        "06" => "Jun",
        "07" => "Jul",
        "08" => "Aug",
        "09" => "Sep",
        "10" => "Oct",
        "11" => "Nov",
        "12" => "Dec",
        _ => return value.to_string(),
    };
    let day = value[8..10].trim_start_matches('0');
    format!("{month} {day}, {year}")
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_graph(series: &[DailyDownloads]) -> String {
    let points = points_for_series(series, SVG_WIDTH, SVG_HEIGHT, SVG_PAD);
    let path = svg_path(&points);
    let area = svg_area(&path, &points);
    let first = series.first().map(|row| row.date.as_str()).unwrap_or("");
    let last = series.last().map(|row| row.date.as_str()).unwrap_or("");
    let band_count = points.len().max(1) as u32;
    let band_width = SVG_WIDTH as f64 / band_count as f64;
    let hits = points
        .iter()
        .zip(series.iter())
        .enumerate()
        .map(|(index, (_, row))| {
            let x = (index as f64 * band_width).floor();
            let width = band_width.ceil() + 1.0;
            let active_points = &points[..=index];
            let active_path = svg_path(active_points);
            let active_area = svg_area(&active_path, active_points);
            let title = format!(
                "{}: {} total downloads",
                format_date(&row.date),
                format_exact_number(row.downloads)
            );
            format!(
                r#"<rect class="downloads-hit" x="{x}" y="0" width="{width}" height="{SVG_HEIGHT}" fill="transparent" stroke="none" pointer-events="all" data-downloads-hover-index="{index}" data-downloads-hover-date="{}" data-downloads-hover-x="{}" data-downloads-hover-y="{}" data-downloads-active-path="{}" data-downloads-active-area="{}"><title>{}</title></rect>"#,
                escape_html(&format_date(&row.date)),
                points[index].0,
                points[index].1,
                escape_html(&active_path),
                escape_html(&active_area),
                escape_html(&title)
            )
        })
        .collect::<String>();

    format!(
        r#"<svg viewBox="0 0 {SVG_WIDTH} {SVG_HEIGHT}" role="img" aria-label="Cumulative Crepuscularity crates.io downloads history"><path class="downloads-area" d="{area}"></path><path class="downloads-line" d="{path}"></path><path class="downloads-area-active" d="" style="display:none"></path><path class="downloads-line-active" d="" style="display:none"></path><circle class="downloads-hover-dot" cx="0" cy="0" r="4" style="display:none"></circle>{hits}</svg><div class="downloads-axis"><span>{}</span><span data-downloads-axis-current-date data-downloads-default-date="{}">{}</span></div>"#,
        escape_html(&format_date(first)),
        escape_html(&format_date(last)),
        escape_html(&format_date(last))
    )
}

fn svg_path(points: &[(u32, u32)]) -> String {
    points
        .iter()
        .enumerate()
        .map(|(index, (x, y))| {
            let command = if index == 0 { "M" } else { "L" };
            format!("{command} {x} {y}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn svg_area(path: &str, points: &[(u32, u32)]) -> String {
    if let Some((last_x, _)) = points.last() {
        format!(
            "{path} L {last_x} {} L {SVG_PAD} {} Z",
            SVG_HEIGHT - SVG_PAD,
            SVG_HEIGHT - SVG_PAD
        )
    } else {
        String::new()
    }
}

fn render_overall_row(total: u64, recent: u64, series: &[DailyDownloads]) -> String {
    format!(
        r#"<div class="downloads-crate-row downloads-overall-row" data-downloads-row-key="__overall"><span><strong>All crepuscularity* crates</strong><small>{} recent downloads</small></span><b data-downloads-values="{}" data-downloads-default="{}" data-downloads-current="{}">{}</b></div>"#,
        format_exact_number(recent),
        values_attr(series),
        format_exact_number(total),
        format_exact_number(total),
        render_static_slot_number(total)
    )
}

fn render_crate_row(crate_downloads: &CrateDownloads) -> String {
    let krate = &crate_downloads.info;
    let version = krate
        .newest_version
        .as_ref()
        .or(krate.max_version.as_ref())
        .or(krate.default_version.as_ref())
        .map(String::as_str)
        .unwrap_or("");

    let total = crate_downloads
        .series
        .last()
        .map(|row| row.downloads)
        .unwrap_or(krate.downloads)
        .max(krate.downloads);

    format!(
        r#"<a class="downloads-crate-row" data-downloads-row-key="{}" href="https://crates.io/crates/{}" target="_blank" rel="noopener noreferrer"><span><strong>{}</strong><small>{}</small></span><b data-downloads-values="{}" data-downloads-default="{}" data-downloads-current="{}">{}</b></a>"#,
        escape_html(&krate.name),
        url_encode_path_segment(&krate.name),
        escape_html(&krate.name),
        escape_html(version),
        values_attr(&crate_downloads.series),
        format_exact_number(total),
        format_exact_number(total),
        render_static_slot_number(total)
    )
}

fn display_recent_downloads(total: u64, _reported_recent: u64) -> u64 {
    total
}

fn values_attr(series: &[DailyDownloads]) -> String {
    escape_html(
        &series
            .iter()
            .map(|row| format_exact_number(row.downloads))
            .collect::<Vec<_>>()
            .join(";"),
    )
}

fn url_encode_path_segment(value: &str) -> String {
    let mut encoded = String::new();

    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }

    encoded
}

#[cfg(target_arch = "wasm32")]
fn set_window_timeout<F>(callback: F, timeout_ms: i32)
where
    F: FnMut() + 'static,
{
    use wasm_bindgen::{closure::Closure, JsCast};
    if let Some(window) = web_sys::window() {
        let closure = Closure::<dyn FnMut()>::wrap(Box::new(callback));
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            timeout_ms,
        );
        closure.forget();
    }
}

#[cfg(target_arch = "wasm32")]
fn set_window_interval<F>(callback: F, timeout_ms: i32)
where
    F: FnMut() + 'static,
{
    use wasm_bindgen::{closure::Closure, JsCast};
    if let Some(window) = web_sys::window() {
        let closure = Closure::<dyn FnMut()>::wrap(Box::new(callback));
        let _ = window.set_interval_with_callback_and_timeout_and_arguments_0(
            closure.as_ref().unchecked_ref(),
            timeout_ms,
        );
        closure.forget();
    }
}

#[cfg(target_arch = "wasm32")]
fn schedule_initial_downloads_refresh(delay_ms: i32, remaining_attempts: u32) {
    set_window_timeout(
        move || {
            if downloads_root().ok().flatten().is_some() {
                wasm_bindgen_futures::spawn_local(async {
                    let _ = refresh_downloads().await;
                });
            } else if remaining_attempts > 0 {
                schedule_initial_downloads_refresh(INITIAL_RETRY_MS, remaining_attempts - 1);
            }
        },
        delay_ms,
    );
}

#[cfg(target_arch = "wasm32")]
fn schedule_downloads_interval() {
    set_window_interval(
        || {
            wasm_bindgen_futures::spawn_local(async {
                let _ = refresh_downloads().await;
            });
        },
        UPDATE_INTERVAL_MS,
    );
}

#[cfg(target_arch = "wasm32")]
async fn refresh_downloads() -> Result<(), JsValue> {
    let Some(root) = downloads_root()? else {
        return Ok(());
    };

    let started_at = js_sys::Date::now();
    match load_downloads().await {
        Ok(data) => {
            let load_ms = (js_sys::Date::now() - started_at).max(0.0).round() as u64;
            render_downloads(&root, &data, load_ms)
        }
        Err(error) => {
            render_error(&root, &format_js_error(&error));
            Ok(())
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn load_downloads() -> Result<DownloadsData, JsValue> {
    let search = fetch_json::<SearchResponse>(SEARCH_URL).await?;
    let crates = filter_crepuscularity_crates(search.crates);
    let payloads = fetch_download_payloads(&crates).await?;
    let series = aggregate_daily_downloads(&payloads);
    let dates = series
        .iter()
        .map(|row| row.date.clone())
        .collect::<Vec<_>>();
    let crates = crates
        .into_iter()
        .zip(payloads.iter())
        .map(|(info, payload)| {
            let daily = aggregate_daily_downloads(std::slice::from_ref(payload));
            let series = cumulative_downloads_at_dates(&daily, info.downloads, &dates);
            CrateDownloads { info, series }
        })
        .collect();

    Ok(DownloadsData { series, crates })
}

#[cfg(target_arch = "wasm32")]
async fn fetch_download_payloads(crates: &[CrateInfo]) -> Result<Vec<DownloadsResponse>, JsValue> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
    let requests = js_sys::Array::new();

    for krate in crates {
        requests.push(&window.fetch_with_str(&absolute_crates_url(
            &krate.links.version_downloads,
        )));
    }

    let responses = JsFuture::from(js_sys::Promise::all(&requests)).await?;
    let responses = js_sys::Array::from(&responses);
    let mut payloads = Vec::with_capacity(responses.length() as usize);

    for response_value in responses.iter() {
        let response: web_sys::Response = response_value.dyn_into()?;
        if !response.ok() {
            return Err(JsValue::from_str(&format!(
                "crates.io returned {}",
                response.status()
            )));
        }
        let json = JsFuture::from(response.json()?).await?;
        payloads.push(decode_json_value::<DownloadsResponse>(&json)?);
    }

    Ok(payloads)
}

#[cfg(target_arch = "wasm32")]
async fn fetch_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, JsValue> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
    let response_value = JsFuture::from(window.fetch_with_str(url)).await?;
    let response: web_sys::Response = response_value.dyn_into()?;

    if !response.ok() {
        return Err(JsValue::from_str(&format!(
            "crates.io returned {}",
            response.status()
        )));
    }

    let json = JsFuture::from(response.json()?).await?;
    decode_json_value(&json)
}

#[cfg(target_arch = "wasm32")]
fn decode_json_value<T: serde::de::DeserializeOwned>(json: &JsValue) -> Result<T, JsValue> {
    let json_string = js_sys::JSON::stringify(json)?
        .as_string()
        .ok_or_else(|| JsValue::from_str("crates.io JSON was not a string"))?;
    serde_json::from_str(&json_string)
        .map_err(|error| JsValue::from_str(&format!("could not parse crates.io JSON: {error}")))
}

#[cfg(target_arch = "wasm32")]
fn downloads_root() -> Result<Option<web_sys::Element>, JsValue> {
    Ok(document()?.query_selector("[data-downloads-widget]")?)
}

#[cfg(target_arch = "wasm32")]
fn document() -> Result<web_sys::Document, JsValue> {
    web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("document unavailable"))
}

#[cfg(target_arch = "wasm32")]
fn render_downloads(
    root: &web_sys::Element,
    data: &DownloadsData,
    load_ms: u64,
) -> Result<(), JsValue> {
    let reported_total = data
        .crates
        .iter()
        .map(|krate| krate.info.downloads)
        .sum::<u64>();
    let reported_recent = data
        .crates
        .iter()
        .map(|krate| krate.info.recent_downloads)
        .sum::<u64>();
    let latest_date = data
        .series
        .last()
        .map(|row| row.date.as_str())
        .unwrap_or("live");
    let cumulative_series = overall_cumulative_downloads(data.series.clone(), reported_total);
    let total = cumulative_series
        .last()
        .map(|row| row.downloads)
        .unwrap_or(reported_total);
    let recent = display_recent_downloads(total, reported_recent);
    let mut list = render_overall_row(total, recent, &cumulative_series);

    for krate in &data.crates {
        list.push_str(&render_crate_row(krate));
    }

    set_slot_number(root, "[data-downloads-total]", total)?;
    set_slot_number(root, "[data-downloads-recent]", recent)?;
    set_slot_number(root, "[data-downloads-crates]", data.crates.len() as u64)?;
    set_text(
        root,
        "[data-downloads-status]",
        &format!(
            "Updated from crates.io through {} · loaded in {}",
            format_date(latest_date),
            format_duration_ms(load_ms)
        ),
    )?;
    set_text(
        root,
        "[data-downloads-peak]",
        &format!("{} total", format_exact_number(total)),
    )?;
    set_html(root, "[data-downloads-graph]", &render_graph(&cumulative_series))?;
    bind_graph_hover(root)?;
    sync_downloads_list(root, &list)?;
    root.set_attribute("data-downloads-state", "ready")?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn set_slot_number(root: &web_sys::Element, selector: &str, value: u64) -> Result<(), JsValue> {
    let Some(element) = root.query_selector(selector)? else {
        return Ok(());
    };
    let value = format_exact_number(value);
    let previous = element.get_attribute("data-downloads-current");
    if previous.as_deref() == Some(value.as_str()) {
        return Ok(());
    }
    element.set_attribute("data-downloads-current", &value)?;
    let html = if previous.is_some() {
        format_slot_transition(previous.as_deref(), &value)
    } else {
        format_slot_transition(None, &value)
    };
    element.set_inner_html(&html);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn sync_downloads_list(root: &web_sys::Element, html: &str) -> Result<(), JsValue> {
    use wasm_bindgen::JsCast;

    let Some(list) = root.query_selector("[data-downloads-list]")? else {
        return Ok(());
    };
    let document = document()?;
    let incoming = document.create_element("div")?;
    incoming.set_inner_html(html);

    let existing_rows = list.children();
    let incoming_rows = incoming.children();

    if existing_rows.length() != incoming_rows.length()
        || existing_rows.length() == 0
        || !download_row_keys_match(&existing_rows, &incoming_rows)
    {
        list.set_inner_html(html);
        return Ok(());
    }

    for index in 0..existing_rows.length() {
        let Some(existing_node) = existing_rows.item(index) else {
            continue;
        };
        let Some(incoming_node) = incoming_rows.item(index) else {
            continue;
        };
        let Ok(existing_row) = existing_node.dyn_into::<web_sys::Element>() else {
            continue;
        };
        let Ok(incoming_row) = incoming_node.dyn_into::<web_sys::Element>() else {
            continue;
        };
        sync_download_row(&existing_row, &incoming_row)?;
    }

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn download_row_keys_match(
    existing_rows: &web_sys::HtmlCollection,
    incoming_rows: &web_sys::HtmlCollection,
) -> bool {
    use wasm_bindgen::JsCast;

    for index in 0..existing_rows.length() {
        let Some(existing_node) = existing_rows.item(index) else {
            return false;
        };
        let Some(incoming_node) = incoming_rows.item(index) else {
            return false;
        };
        let Ok(existing_row) = existing_node.dyn_into::<web_sys::Element>() else {
            return false;
        };
        let Ok(incoming_row) = incoming_node.dyn_into::<web_sys::Element>() else {
            return false;
        };
        if existing_row.get_attribute("data-downloads-row-key")
            != incoming_row.get_attribute("data-downloads-row-key")
        {
            return false;
        }
    }

    true
}

#[cfg(target_arch = "wasm32")]
fn sync_download_row(
    existing_row: &web_sys::Element,
    incoming_row: &web_sys::Element,
) -> Result<(), JsValue> {
    for attr in ["href", "target", "rel"] {
        if let Some(value) = incoming_row.get_attribute(attr) {
            existing_row.set_attribute(attr, &value)?;
        }
    }

    if let (Some(existing_label), Some(incoming_label)) = (
        existing_row.query_selector("span")?,
        incoming_row.query_selector("span")?,
    ) {
        existing_label.set_inner_html(&incoming_label.inner_html());
    }

    let (Some(existing_value), Some(incoming_value)) = (
        existing_row.query_selector("b")?,
        incoming_row.query_selector("b")?,
    ) else {
        return Ok(());
    };
    let Some(default_value) = incoming_value.get_attribute("data-downloads-default") else {
        return Ok(());
    };
    let previous = existing_value.get_attribute("data-downloads-current");

    if let Some(values) = incoming_value.get_attribute("data-downloads-values") {
        existing_value.set_attribute("data-downloads-values", &values)?;
    }
    existing_value.set_attribute("data-downloads-default", &default_value)?;
    if previous.as_deref() == Some(default_value.as_str()) {
        return Ok(());
    }

    existing_value.set_attribute("data-downloads-current", &default_value)?;
    existing_value.set_inner_html(&format_slot_transition(
        previous.as_deref(),
        &default_value,
    ));

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn bind_graph_hover(root: &web_sys::Element) -> Result<(), JsValue> {
    use wasm_bindgen::{closure::Closure, JsCast};

    let Some(graph) = root.query_selector("[data-downloads-graph]")? else {
        return Ok(());
    };

    let move_root = root.clone();
    let move_graph = graph.clone();
    let on_move = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(
        move |event: web_sys::Event| {
            let Ok(pointer) = event.dyn_into::<web_sys::PointerEvent>() else {
                return;
            };
            if !should_track_graph_pointer(&move_root, &pointer) {
                return;
            }
            pointer.prevent_default();
            let _ = apply_graph_pointer_event(&move_root, &move_graph, &pointer);
        },
    ));
    graph.add_event_listener_with_callback("pointermove", on_move.as_ref().unchecked_ref())?;
    on_move.forget();

    let down_root = root.clone();
    let down_graph = graph.clone();
    let on_down = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(
        move |event: web_sys::Event| {
            let Ok(pointer) = event.dyn_into::<web_sys::PointerEvent>() else {
                return;
            };
            pointer.prevent_default();
            let _ = down_graph.set_pointer_capture(pointer.pointer_id());
            let _ = set_graph_scrubbing(&down_root, true);
            let _ = apply_graph_pointer_event(&down_root, &down_graph, &pointer);
        },
    ));
    graph.add_event_listener_with_callback("pointerdown", on_down.as_ref().unchecked_ref())?;
    on_down.forget();

    let up_root = root.clone();
    let up_graph = graph.clone();
    let on_up = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(
        move |event: web_sys::Event| {
            let Ok(pointer) = event.dyn_into::<web_sys::PointerEvent>() else {
                return;
            };
            let _ = up_graph.release_pointer_capture(pointer.pointer_id());
            let _ = set_graph_scrubbing(&up_root, false);
        },
    ));
    graph.add_event_listener_with_callback("pointerup", on_up.as_ref().unchecked_ref())?;
    on_up.forget();

    let cancel_root = root.clone();
    let cancel_graph = graph.clone();
    let on_cancel = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(
        move |event: web_sys::Event| {
            let Ok(pointer) = event.dyn_into::<web_sys::PointerEvent>() else {
                return;
            };
            let _ = cancel_graph.release_pointer_capture(pointer.pointer_id());
            let _ = set_graph_scrubbing(&cancel_root, false);
        },
    ));
    graph.add_event_listener_with_callback("pointercancel", on_cancel.as_ref().unchecked_ref())?;
    on_cancel.forget();

    let leave_root = root.clone();
    let on_leave = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(
        move |event: web_sys::Event| {
            if graph_is_scrubbing(&leave_root) {
                return;
            }
            let pointer_type = event
                .dyn_ref::<web_sys::PointerEvent>()
                .map(|pointer| pointer.pointer_type())
                .unwrap_or_else(|| "mouse".into());
            if pointer_type == "mouse" {
                let _ = reset_graph_hover(&leave_root);
            }
        },
    ));
    graph.add_event_listener_with_callback("pointerleave", on_leave.as_ref().unchecked_ref())?;
    on_leave.forget();

    bind_graph_outside_reset(root)?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn graph_is_scrubbing(root: &web_sys::Element) -> bool {
    root.get_attribute("data-downloads-scrubbing").as_deref() == Some("true")
}

#[cfg(target_arch = "wasm32")]
fn set_graph_scrubbing(root: &web_sys::Element, scrubbing: bool) -> Result<(), JsValue> {
    if scrubbing {
        root.set_attribute("data-downloads-scrubbing", "true")
    } else {
        root.remove_attribute("data-downloads-scrubbing")
    }
}

#[cfg(target_arch = "wasm32")]
fn should_track_graph_pointer(root: &web_sys::Element, pointer: &web_sys::PointerEvent) -> bool {
    if graph_is_scrubbing(root) {
        return true;
    }
    pointer.pointer_type() == "mouse"
}

#[cfg(target_arch = "wasm32")]
fn apply_graph_pointer_event(
    root: &web_sys::Element,
    graph: &web_sys::Element,
    pointer: &web_sys::PointerEvent,
) -> Result<(), JsValue> {
    let Some(hit) = graph_hit_at_pointer(graph, pointer) else {
        return Ok(());
    };
    apply_graph_hit(root, &hit)
}

#[cfg(target_arch = "wasm32")]
fn graph_hit_at_pointer(
    graph: &web_sys::Element,
    pointer: &web_sys::PointerEvent,
) -> Option<web_sys::Element> {
    let doc = document().ok()?;
    let element = doc.element_from_point(pointer.client_x() as f32, pointer.client_y() as f32)?;
    let hit = element.closest(".downloads-hit").ok().flatten()?;
    let graph_container = hit.closest("[data-downloads-graph]").ok().flatten()?;
    if graph_container == *graph {
        Some(hit)
    } else {
        None
    }
}

#[cfg(target_arch = "wasm32")]
fn apply_graph_hit(root: &web_sys::Element, hit: &web_sys::Element) -> Result<(), JsValue> {
    let Some(index) = hit
        .get_attribute("data-downloads-hover-index")
        .and_then(|value| value.parse::<usize>().ok())
    else {
        return Ok(());
    };
    graph_hover_dot(root, hit)?;
    graph_hover_paths(root, hit)?;
    update_graph_hover_date(root, hit)?;
    update_download_values(root, index)?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn bind_graph_outside_reset(root: &web_sys::Element) -> Result<(), JsValue> {
    use wasm_bindgen::{closure::Closure, JsCast};

    if root.get_attribute("data-downloads-outside-reset-bound").is_some() {
        return Ok(());
    }
    root.set_attribute("data-downloads-outside-reset-bound", "true")?;

    let reset_root = root.clone();
    let on_down = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(
        move |event: web_sys::Event| {
            if graph_is_scrubbing(&reset_root) {
                return;
            }
            let inside_graph = event
                .target()
                .and_then(|target: web_sys::EventTarget| {
                    target.dyn_into::<web_sys::Element>().ok()
                })
                .and_then(|target| target.closest("[data-downloads-graph]").ok().flatten())
                .is_some();
            if !inside_graph {
                let _ = reset_graph_hover(&reset_root);
            }
        },
    ));
    document()?.add_event_listener_with_callback("pointerdown", on_down.as_ref().unchecked_ref())?;
    on_down.forget();

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn reset_graph_hover(root: &web_sys::Element) -> Result<(), JsValue> {
    hide_graph_hover(root)?;
    restore_graph_hover_date(root)?;
    restore_download_values(root)
}

#[cfg(target_arch = "wasm32")]
fn graph_hover_dot(root: &web_sys::Element, target: &web_sys::Element) -> Result<(), JsValue> {
    let Some(dot) = root.query_selector(".downloads-hover-dot")? else {
        return Ok(());
    };
    if let Some(x) = target.get_attribute("data-downloads-hover-x") {
        dot.set_attribute("cx", &x)?;
    }
    if let Some(y) = target.get_attribute("data-downloads-hover-y") {
        dot.set_attribute("cy", &y)?;
    }
    dot.set_attribute("style", "display:block")?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn graph_hover_paths(root: &web_sys::Element, target: &web_sys::Element) -> Result<(), JsValue> {
    if let Some(graph) = root.query_selector("[data-downloads-graph]")? {
        graph.set_attribute("data-downloads-graph-hovering", "true")?;
    }
    if let Some(area) = root.query_selector(".downloads-area-active")? {
        if let Some(path) = target.get_attribute("data-downloads-active-area") {
            area.set_attribute("d", &path)?;
            area.set_attribute("style", "display:block")?;
        }
    }
    if let Some(line) = root.query_selector(".downloads-line-active")? {
        if let Some(path) = target.get_attribute("data-downloads-active-path") {
            line.set_attribute("d", &path)?;
            line.set_attribute("style", "display:block")?;
        }
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn hide_graph_hover(root: &web_sys::Element) -> Result<(), JsValue> {
    if let Some(dot) = root.query_selector(".downloads-hover-dot")? {
        dot.set_attribute("style", "display:none")?;
    }
    if let Some(area) = root.query_selector(".downloads-area-active")? {
        area.set_attribute("style", "display:none")?;
    }
    if let Some(line) = root.query_selector(".downloads-line-active")? {
        line.set_attribute("style", "display:none")?;
    }
    if let Some(graph) = root.query_selector("[data-downloads-graph]")? {
        graph.remove_attribute("data-downloads-graph-hovering")?;
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn update_graph_hover_date(
    root: &web_sys::Element,
    target: &web_sys::Element,
) -> Result<(), JsValue> {
    let Some(date) = target.get_attribute("data-downloads-hover-date") else {
        return Ok(());
    };
    set_text(root, "[data-downloads-axis-current-date]", &date)
}

#[cfg(target_arch = "wasm32")]
fn restore_graph_hover_date(root: &web_sys::Element) -> Result<(), JsValue> {
    let Some(label) = root.query_selector("[data-downloads-axis-current-date]")? else {
        return Ok(());
    };
    if let Some(date) = label.get_attribute("data-downloads-default-date") {
        label.set_text_content(Some(&date));
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn update_download_values(root: &web_sys::Element, index: usize) -> Result<(), JsValue> {
    use wasm_bindgen::JsCast;

    let values = root.query_selector_all("[data-downloads-values]")?;
    for item_index in 0..values.length() {
        let Some(node) = values.item(item_index) else {
            continue;
        };
        let Ok(element) = node.dyn_into::<web_sys::Element>() else {
            continue;
        };
        let Some(raw_values) = element.get_attribute("data-downloads-values") else {
            continue;
        };
        let Some(value) = raw_values.split(';').nth(index) else {
            continue;
        };
        let previous = element.get_attribute("data-downloads-current");
        if previous.as_deref() == Some(value) {
            continue;
        }
        element.set_attribute("data-downloads-current", value)?;
        element.set_inner_html(&format_slot_transition(previous.as_deref(), value));
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn restore_download_values(root: &web_sys::Element) -> Result<(), JsValue> {
    use wasm_bindgen::JsCast;

    let values = root.query_selector_all("[data-downloads-values]")?;
    for item_index in 0..values.length() {
        let Some(node) = values.item(item_index) else {
            continue;
        };
        let Ok(element) = node.dyn_into::<web_sys::Element>() else {
            continue;
        };
        let Some(value) = element.get_attribute("data-downloads-default") else {
            continue;
        };
        let previous = element.get_attribute("data-downloads-current");
        if previous.as_deref() == Some(value.as_str()) {
            continue;
        }
        element.set_attribute("data-downloads-current", &value)?;
        element.set_inner_html(&format_slot_transition(previous.as_deref(), &value));
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn render_error(root: &web_sys::Element, error: &str) {
    let _ = set_text(
        root,
        "[data-downloads-status]",
        &format!("crates.io data unavailable: {error}"),
    );
    let _ = root.set_attribute("data-downloads-state", "error");
}

#[cfg(target_arch = "wasm32")]
fn set_text(root: &web_sys::Element, selector: &str, value: &str) -> Result<(), JsValue> {
    if let Some(element) = root.query_selector(selector)? {
        element.set_text_content(Some(value));
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn set_html(root: &web_sys::Element, selector: &str, value: &str) -> Result<(), JsValue> {
    if let Some(element) = root.query_selector(selector)? {
        element.set_inner_html(value);
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn absolute_crates_url(path_or_url: &str) -> String {
    if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
        path_or_url.to_string()
    } else {
        format!("{API_BASE}{path_or_url}")
    }
}

#[cfg(target_arch = "wasm32")]
fn format_js_error(error: &JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| "unknown error".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crate_info(name: &str, downloads: u64) -> CrateInfo {
        CrateInfo {
            name: name.to_string(),
            downloads,
            recent_downloads: 0,
            newest_version: None,
            max_version: None,
            default_version: None,
            links: CrateLinks {
                version_downloads: format!("/api/v1/crates/{name}/downloads"),
            },
        }
    }

    #[test]
    fn keeps_crepuscularity_prefixed_crates() {
        let crates = filter_crepuscularity_crates(vec![
            crate_info("crepuscularity", 8),
            crate_info("crepuscularity-core", 11),
            crate_info("not-crepuscularity", 99),
            crate_info("crepuscularity_lite", 3),
        ]);

        let names = crates
            .into_iter()
            .map(|krate| krate.name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "crepuscularity-core",
                "crepuscularity",
                "crepuscularity_lite"
            ]
        );
    }

    #[test]
    fn aggregates_version_and_extra_downloads() {
        let series = aggregate_daily_downloads(&[
            DownloadsResponse {
                version_downloads: vec![
                    DownloadRow {
                        date: "2026-06-01".to_string(),
                        downloads: 4,
                    },
                    DownloadRow {
                        date: "2026-06-01".to_string(),
                        downloads: 6,
                    },
                    DownloadRow {
                        date: "2026-06-02".to_string(),
                        downloads: 3,
                    },
                ],
                meta: DownloadsMeta {
                    extra_downloads: vec![DownloadRow {
                        date: "2026-06-02".to_string(),
                        downloads: 2,
                    }],
                },
            },
            DownloadsResponse {
                version_downloads: vec![DownloadRow {
                    date: "2026-06-01".to_string(),
                    downloads: 5,
                }],
                meta: DownloadsMeta::default(),
            },
        ]);

        assert_eq!(
            series,
            vec![
                DailyDownloads {
                    date: "2026-06-01".to_string(),
                    downloads: 15,
                },
                DailyDownloads {
                    date: "2026-06-02".to_string(),
                    downloads: 5,
                },
            ]
        );
    }

    #[test]
    fn maps_dated_totals_to_svg_points() {
        let points = points_for_series(
            &[
                DailyDownloads {
                    date: "2026-06-01".to_string(),
                    downloads: 0,
                },
                DailyDownloads {
                    date: "2026-06-02".to_string(),
                    downloads: 10,
                },
                DailyDownloads {
                    date: "2026-06-03".to_string(),
                    downloads: 5,
                },
            ],
            300,
            120,
            16,
        );

        assert_eq!(points, vec![(16, 104), (150, 16), (284, 60)]);
    }

    #[test]
    fn turns_daily_downloads_into_cumulative_totals() {
        let series = cumulative_downloads([
            DailyDownloads {
                date: "2026-06-01".to_string(),
                downloads: 4,
            },
            DailyDownloads {
                date: "2026-06-02".to_string(),
                downloads: 6,
            },
            DailyDownloads {
                date: "2026-06-03".to_string(),
                downloads: 2,
            },
        ]);

        assert_eq!(
            series,
            vec![
                DailyDownloads {
                    date: "2026-06-01".to_string(),
                    downloads: 4,
                },
                DailyDownloads {
                    date: "2026-06-02".to_string(),
                    downloads: 10,
                },
                DailyDownloads {
                    date: "2026-06-03".to_string(),
                    downloads: 12,
                },
            ]
        );
    }

    #[test]
    fn anchors_visible_cumulative_history_to_all_time_total() {
        let series = overall_cumulative_downloads(
            [
                DailyDownloads {
                    date: "2026-06-01".to_string(),
                    downloads: 4,
                },
                DailyDownloads {
                    date: "2026-06-02".to_string(),
                    downloads: 6,
                },
            ],
            100,
        );

        assert_eq!(
            series,
            vec![
                DailyDownloads {
                    date: "2026-06-01".to_string(),
                    downloads: 94,
                },
                DailyDownloads {
                    date: "2026-06-02".to_string(),
                    downloads: 100,
                },
            ]
        );
    }

    #[test]
    fn lets_daily_cumulative_history_advance_past_stale_reported_total() {
        let series = overall_cumulative_downloads(
            [
                DailyDownloads {
                    date: "2026-06-05".to_string(),
                    downloads: 4,
                },
                DailyDownloads {
                    date: "2026-06-06".to_string(),
                    downloads: 6,
                },
            ],
            8,
        );

        assert_eq!(
            series,
            vec![
                DailyDownloads {
                    date: "2026-06-05".to_string(),
                    downloads: 4,
                },
                DailyDownloads {
                    date: "2026-06-06".to_string(),
                    downloads: 10,
                },
            ]
        );
    }

    #[test]
    fn scales_high_cumulative_ranges_by_visible_delta() {
        let points = points_for_series(
            &[
                DailyDownloads {
                    date: "2026-06-01".to_string(),
                    downloads: 94,
                },
                DailyDownloads {
                    date: "2026-06-02".to_string(),
                    downloads: 100,
                },
            ],
            300,
            120,
            16,
        );

        assert_eq!(points, vec![(16, 104), (284, 16)]);
    }

    #[test]
    fn formats_exact_download_totals() {
        assert_eq!(format_exact_number(999), "999");
        assert_eq!(format_exact_number(1_200), "1 200");
        assert_eq!(format_exact_number(1_200_000), "1 200 000");
        assert_eq!(format_duration_ms(875), "875 ms");
        assert_eq!(format_duration_ms(1_250), "1.2 s");
        assert!(render_slot_number(1_200).contains("downloads-number-slot"));
    }

    #[test]
    fn slot_transition_only_animates_changed_digits() {
        let unchanged = format_slot_transition(Some("4 519"), "4 519");
        assert!(!unchanged.contains("downloads-slot-changed"));

        let changed = format_slot_transition(Some("4 519"), "4 529");
        assert_eq!(changed.matches("downloads-slot-changed").count(), 1);
    }

    #[test]
    fn displays_recent_downloads_from_corrected_total() {
        assert_eq!(display_recent_downloads(4_542, 4_519), 4_542);
    }

    #[test]
    fn renders_cumulative_graph_label() {
        let graph = render_graph(&[DailyDownloads {
            date: "2026-06-01".to_string(),
            downloads: 4,
        }]);

        assert!(graph.contains("Cumulative Crepuscularity crates.io downloads history"));
        assert!(graph.contains("Jun 1, 2026"));
        assert!(graph.contains("downloads-hit"));
        assert!(graph.contains("4 total downloads"));
    }
}
