//! Color scheme.

use epaint::{Color32, FontFamily};

use crate::types::IconRef;

/// Theme colors and font. Read at draw time; change it through
/// [`crate::Ui::set_scheme`] / [`crate::Ui::update_scheme`].
#[derive(Clone, Debug, PartialEq)]
pub struct Scheme {
    pub background: Color32,
    pub main: Color32,
    pub accent: Color32,
    pub outline: Color32,
    pub font_color: Color32,
    pub font: FontFamily,
    pub red: Color32,
    pub blue: Color32,
    pub destructive: Color32,
    pub dark: Color32,
    pub white: Color32,
    pub background_image: Option<IconRef>,
    /// Flips the direction of [`crate::color::better`].
    pub is_light: bool,
}

impl Default for Scheme {
    fn default() -> Self {
        Self {
            background: Color32::from_rgb(15, 15, 15),
            main: Color32::from_rgb(25, 25, 25),
            accent: Color32::from_rgb(125, 85, 255),
            outline: Color32::from_rgb(40, 40, 40),
            font_color: Color32::WHITE,
            font: FontFamily::Monospace,
            red: Color32::from_rgb(255, 50, 50),
            blue: Color32::from_rgb(80, 155, 255),
            destructive: Color32::from_rgb(220, 38, 38),
            dark: Color32::BLACK,
            white: Color32::WHITE,
            background_image: None,
            is_light: false,
        }
    }
}

impl Scheme {
    /// `GetBetterColor(color, add)` with this scheme's light/dark direction.
    pub fn better(&self, c: Color32, add: f32) -> Color32 {
        crate::color::better(c, add, self.is_light)
    }

    /// Placeholder text color: font color with half the HSV value.
    pub fn placeholder(&self) -> Color32 {
        crate::color::darker(self.font_color)
    }
}
