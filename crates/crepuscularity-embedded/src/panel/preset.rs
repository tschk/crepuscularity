use crate::{PanelConfig, ScreenSize};

/// Common SPI TFT modules — maps to [`PanelConfig`] and size for [`crate::Ui`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPreset {
    Ili9341_240x320,
    St7789_240x320,
    St7789_240x280,
    St7789_240x135,
}

impl PanelPreset {
    pub const fn size(self) -> ScreenSize {
        match self {
            Self::Ili9341_240x320 | Self::St7789_240x320 => ScreenSize::new(240, 320),
            Self::St7789_240x280 => ScreenSize::new(240, 280),
            Self::St7789_240x135 => ScreenSize::new(240, 135),
        }
    }

    pub const fn width(self) -> u16 {
        self.size().width
    }

    pub const fn height(self) -> u16 {
        self.size().height
    }

    pub const fn config(self) -> PanelConfig {
        match self {
            Self::Ili9341_240x320 => PanelConfig::ili9341_240x320(),
            Self::St7789_240x320 | Self::St7789_240x280 | Self::St7789_240x135 => {
                PanelConfig::st7789_240x320()
            }
        }
    }

    pub const fn byte_count(self) -> usize {
        let s = self.size();
        s.pixel_count() * 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PanelConfig, Rgb565ByteOrder};

    #[test]
    fn ili9341_preset() {
        let p = PanelPreset::Ili9341_240x320;
        assert_eq!(p.width(), 240);
        assert_eq!(p.height(), 320);
        assert_eq!(p.byte_count(), 240 * 320 * 2);
        assert_eq!(p.config(), PanelConfig::ili9341_240x320());
        assert_eq!(p.config().byte_order, Rgb565ByteOrder::Rgb);
    }

    #[test]
    fn st7789_preset_bgr() {
        let p = PanelPreset::St7789_240x320;
        assert_eq!(p.config().byte_order, Rgb565ByteOrder::Bgr);
    }
}
