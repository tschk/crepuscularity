//! Pure Bayer 4×4 sparkline math for `crepuscularity-gpui`.
//!
//! No GPUI dependency — returns cell alphas as `Vec<u8>` (0–255) in row-major
//! order (`y * cols + x`). Thresholds match the moonshine/svelte ports:
//! `(bayer_index + 0.5) / 16`.

/// Classic Bayer 4×4 indices, normalized with half-step centering.
pub const BAYER: [[f32; 4]; 4] = [
    [
        (0.0 + 0.5) / 16.0,
        (8.0 + 0.5) / 16.0,
        (2.0 + 0.5) / 16.0,
        (10.0 + 0.5) / 16.0,
    ],
    [
        (12.0 + 0.5) / 16.0,
        (4.0 + 0.5) / 16.0,
        (14.0 + 0.5) / 16.0,
        (6.0 + 0.5) / 16.0,
    ],
    [
        (3.0 + 0.5) / 16.0,
        (11.0 + 0.5) / 16.0,
        (1.0 + 0.5) / 16.0,
        (9.0 + 0.5) / 16.0,
    ],
    [
        (15.0 + 0.5) / 16.0,
        (7.0 + 0.5) / 16.0,
        (13.0 + 0.5) / 16.0,
        (5.0 + 0.5) / 16.0,
    ],
];

pub const CELL: f32 = 2.0;
pub const MAX_COLS: usize = 520;
pub const MAX_ROWS: usize = 200;
pub const BORDER_ALPHA: f32 = 0.72;
pub const OFF_TIER: f32 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Gradient,
    Dotted,
    Hatched,
    Solid,
}

#[inline]
pub fn clamp01(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

/// Backing grid size for a logical width/height (same rules as JS/Flutter).
pub fn backing_size(width: f32, height: f32) -> (usize, usize) {
    let cols = ((width / CELL).round() as isize).clamp(8, MAX_COLS as isize) as usize;
    let rows = ((height / CELL).round() as isize).clamp(8, MAX_ROWS as isize) as usize;
    (cols, rows)
}

/// Linear resample of a series onto `cols` samples.
pub fn resample(source: &[f32], cols: usize) -> Vec<f32> {
    if cols == 0 {
        return Vec::new();
    }
    if source.is_empty() {
        return vec![0.0; cols];
    }
    let last = (source.len() - 1).max(1) as f32;
    let mut out = vec![0.0; cols];
    for (c, out_c) in out.iter_mut().enumerate().take(cols) {
        let t = (c as f32 / (cols - 1).max(1) as f32) * last;
        let i = t.floor() as usize;
        let f = t - i as f32;
        let a = source[i];
        let b = source[(i + 1).min(source.len() - 1)];
        *out_c = a + (b - a) * f;
    }
    out
}

fn column_alphas(
    top: usize,
    floor: usize,
    x: usize,
    rows: usize,
    variant: Variant,
    intensity: f32,
) -> Vec<(usize, u8)> {
    let mut cells = Vec::new();
    if top >= floor {
        let a = (clamp01(BORDER_ALPHA) * 255.0).round() as u8;
        if top < rows {
            cells.push((top, a));
        }
        return cells;
    }
    let depth = floor - top;
    let bias = match variant {
        Variant::Dotted => 0.12,
        _ => 0.0,
    };
    for y in top..floor {
        let density = (y - top) as f32 / depth as f32;
        if matches!(variant, Variant::Hatched) && ((x + y) & 3) >= 2 {
            continue;
        }
        let threshold = BAYER[y & 3][x & 3] - 0.1 * intensity - bias;
        let lit = matches!(variant, Variant::Solid) || density > threshold;
        if matches!(variant, Variant::Dotted) && !lit {
            continue;
        }
        let k = (0.3 + density * 0.7) * (1.0 + 0.22 * intensity);
        let alpha = clamp01(if lit { k } else { k * OFF_TIER });
        cells.push((y, (alpha * 255.0).round() as u8));
    }
    // Top edge highlight
    cells.push((top, (clamp01(BORDER_ALPHA) * 255.0).round() as u8));
    if depth > 1 {
        cells.push((top + 1, (clamp01(BORDER_ALPHA * 0.5) * 255.0).round() as u8));
    }
    cells
}

/// Compute row-major cell alphas for a dithered sparkline fill.
///
/// Length is always `cols * rows`. Empty / unused cells are `0`.
pub fn sparkline_alphas(
    values: &[f32],
    cols: usize,
    rows: usize,
    variant: Variant,
    intensity: f32,
) -> Vec<u8> {
    let mut out = vec![0u8; cols.saturating_mul(rows)];
    if values.is_empty() || cols == 0 || rows == 0 {
        return out;
    }
    let min = values.iter().copied().fold(f32::INFINITY, f32::min);
    let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let span = (max - min).max(1e-9);
    let resampled = resample(values, cols);
    let floor = rows - 1;
    for x in 0..cols {
        let normalized = (resampled[x] - min) / span;
        let top = ((rows - 1) as f32 - normalized * (rows - 1) as f32)
            .round()
            .clamp(0.0, (rows - 1) as f32) as usize;
        for (y, a) in column_alphas(top, floor, x, rows, variant, intensity) {
            if y < rows {
                // Later writes (edge highlights) win — same order as paint.
                out[y * cols + x] = a;
            }
        }
    }
    out
}

/// Convenience: derive backing size from logical size, then compute alphas.
pub fn sparkline_alphas_for_size(
    values: &[f32],
    width: f32,
    height: f32,
    variant: Variant,
    intensity: f32,
) -> (usize, usize, Vec<u8>) {
    let (cols, rows) = backing_size(width, height);
    let alphas = sparkline_alphas(values, cols, rows, variant, intensity);
    (cols, rows, alphas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bayer_first_cell() {
        assert!((BAYER[0][0] - 0.5 / 16.0).abs() < 1e-6);
        assert!((BAYER[1][0] - 12.5 / 16.0).abs() < 1e-6);
    }

    #[test]
    fn resample_identity() {
        assert_eq!(resample(&[1.0, 2.0, 3.0], 3), vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn sparkline_nonzero() {
        let alphas = sparkline_alphas(&[1.0, 5.0, 1.0], 12, 16, Variant::Gradient, 0.0);
        assert_eq!(alphas.len(), 12 * 16);
        assert!(alphas.iter().any(|&a| a > 0));
        assert!(alphas.iter().any(|&a| a == 0));
    }

    #[test]
    fn backing_size_clamps() {
        let (c, r) = backing_size(100.0, 40.0);
        assert_eq!((c, r), (50, 20));
    }
}
