use lazy_static;
use std::cmp::min;

fn map<T, U, F: Fn(&T) -> U>(tt: &[T; 4], f: F) -> [U; 4] {
    [f(&tt[0]), f(&tt[1]), f(&tt[2]), f(&tt[3])]
}
fn bimap<T, U, V, F: Fn(&T, &U) -> V>(tt: &[T; 4], uu: &[U; 4], f: F) -> [V; 4] {
    [
        f(&tt[0], &uu[0]),
        f(&tt[1], &uu[1]),
        f(&tt[2], &uu[2]),
        f(&tt[3], &uu[3]),
    ]
}
fn pairmap<T, U, F: Fn(&T, &T) -> U>(tt: &[T; 8], f: F) -> [U; 4] {
    [
        f(&tt[0], &tt[1]),
        f(&tt[2], &tt[3]),
        f(&tt[4], &tt[5]),
        f(&tt[6], &tt[7]),
    ]
}
fn hex_char(c: char) -> Result<u8, &'static str> {
    match c {
        '0'..='9' | 'a'..='f' | 'A'..='F' => Ok(c as u8),
        _ => Err("Non-hex char passed off as hex char (0-9,a-z,A-Z)."),
    }
}
fn hex_val(b: &u8) -> Result<u8, &'static str> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err("Non-hex byte passed off as hex byte."),
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Color([u8; 4]);

lazy_static::lazy_static! {
    // reds
    pub static ref INDIAN_RED: Color = Color::hex_bb(b"#cd5c5c").expect("INDIAN_RED color parse error.");
    pub static ref LIGHT_CORAL: Color = Color::hex_bb(b"#f08080").expect("LIGHT_CORAL color parse error.");
    pub static ref SALMON: Color = Color::hex_bb(b"#fa8072").expect("SALMON color parse error.");
    pub static ref DARK_SALMON: Color = Color::hex_bb(b"#e9967a").expect("DARK_SALMON color parse error.");
    pub static ref LIGHT_SALMON: Color = Color::hex_bb(b"#ffa07a").expect("LIGHT_SALMON color parse error.");
    pub static ref CRIMSON: Color = Color::hex_bb(b"#dc143c").expect("CRIMSON color parse error.");
    pub static ref RED: Color =  Color::hex_bb(b"#ff0000").expect("RED color parse error.");
    pub static ref FIRE_BRICK: Color =  Color::hex_bb(b"#b22222").expect("FIRE_BRICK color parse error.");
    pub static ref DARK_RED: Color =  Color::hex_bb(b"#8b0000").expect("DARK_RED color parse error.");
    // pinks
    pub static ref PINK: Color = Color::hex_bb(b"#ffc0cb").expect("PINK color parse error.");
    pub static ref LIGHT_PINK: Color = Color::hex_bb(b"#ffb6c1").expect("LIGHT)PINK color parse error.");
    pub static ref HOT_PINK: Color = Color::hex_bb(b"#ff69b4").expect("HOT_PINK color parse error.");
    pub static ref DEEP_PINK: Color = Color::hex_bb(b"#ff1493").expect("DEEP_PINK color parse error.");
    pub static ref MEDIUM_VIOLET_RED: Color = Color::hex_bb(b"#c71585").expect("MEDIUM_VIOLET_RED color parse error.");
    pub static ref PALE_VIOLET_RED: Color = Color::hex_bb(b"#db7093").expect("PALE_VIOLET_RED color parse error.");
    // orange
    pub static ref CORAL: Color = Color::hex_bb(b"#ff7f50").expect("CORAL color parse error.");
    pub static ref TOMATO: Color = Color::hex_bb(b"#ff6347").expect("TOMATO color parse error.");
    pub static ref ORANGE_RED: Color = Color::hex_bb(b"#ff4500").expect("ORANGE_RED color parse error.");
    pub static ref DARK_ORANGE: Color = Color::hex_bb(b"#ff8c00").expect("DARK_ORANGE color parse error.");
    pub static ref ORANGE: Color = Color::hex_bb(b"#ffa500").expect("ORANGE color parse error.");
    // yellows
    pub static ref GOLD: Color = Color::hex_bb(b"#ffd700").expect("GOLD color parse error.");
    pub static ref YELLOW: Color = Color::hex_bb(b"#ffff00").expect("YELLOW color parse error.");
    pub static ref LIGHT_YELLOW: Color = Color::hex_bb(b"#ffffe0").expect("LIGHT_YELLOW color parse error.");
    pub static ref LEMON_CHIFFON: Color = Color::hex_bb(b"#fffacd").expect("LEMON_CHIFFON color parse error.");
    pub static ref LIGHT_GOLDEN_ROD_YELLOW: Color = Color::hex_bb(b"#fafad2").expect("LIGHT_GOLDEN_ROD_YELLOW color parse error.");
    pub static ref PAPAYA_WHIP: Color = Color::hex_bb(b"#ffefd5").expect("PAPAYA_WHIP color parse error.");
    pub static ref MOCCASIN: Color = Color::hex_bb(b"#ffe4b5").expect("MOCCASIN color parse error.");
    pub static ref PEACH_PUFF: Color = Color::hex_bb(b"#ffdab9").expect("PEACH_PUFF color parse error.");
    pub static ref PALE_GOLDEN_ROD: Color = Color::hex_bb(b"#eee8aa").expect("PALE_GOLDEN_ROD color parse error.");
    pub static ref KHAKI: Color = Color::hex_bb(b"#f0e68c").expect("KHAKI color parse error.");
    pub static ref DARK_KHAKI: Color = Color::hex_bb(b"#bdb76b").expect("DARK_KHAKI color parse error.");
    // purples
    pub static ref LAVENDER: Color = Color::hex_bb(b"#e6e6fa").expect("LAVENDER color parse error.");
    pub static ref THISTLE: Color = Color::hex_bb(b"#d8bfd8").expect("THISTLE color parse error.");
    pub static ref PLUM: Color = Color::hex_bb(b"#dda0dd").expect("PLUM color parse error.");
    pub static ref VIOLET: Color = Color::hex_bb(b"#ee82ee").expect("VIOLET color parse error.");
    pub static ref ORCHID: Color = Color::hex_bb(b"#da70d6").expect("ORCHID color parse error.");
    pub static ref FUSCHIA: Color = Color::hex_bb(b"#ff00ff").expect("FUSCHIA color parse error.");
    pub static ref MAGENTA: Color = Color::hex_bb(b"#ff00ff").expect("MAGENTA color parse error.");
    pub static ref MEDIUM_ORCHID: Color = Color::hex_bb(b"#ba55d3").expect("MEDIUM_ORCHID color parse error.");
    pub static ref MEDIUM_PURPLE: Color = Color::hex_bb(b"#9370db").expect("MEDIUM_PURPLE color parse error.");
    pub static ref REBECCA_PURPLE: Color = Color::hex_bb(b"#ff3399").expect("REBECCA_PURPLE color parse error.");
    pub static ref BLUE_VIOLET: Color = Color::hex_bb(b"#8a2be2").expect("BLUE_VIOLET color parse error.");
    pub static ref DARK_VIOLET: Color = Color::hex_bb(b"#9400d3").expect("DARK_VIOLET color parse error.");
    pub static ref DARK_ORCHID: Color = Color::hex_bb(b"#9932cc").expect("DARK_ORCHID color parse error.");
    pub static ref DARK_MAGENTA: Color = Color::hex_bb(b"#8b008b").expect("DARK_MAGENTA color parse error.");
    pub static ref PURPLE: Color = Color::hex_bb(b"#800080").expect("PURPLE color parse error.");
    pub static ref INDIGO: Color = Color::hex_bb(b"#4b0082").expect("INDIGO color parse error.");
    pub static ref SLATE_BLUE: Color = Color::hex_bb(b"#6a5acd").expect("SLATE_BLUE color parse error.");
    pub static ref DARK_SLATE_BLUE: Color = Color::hex_bb(b"#483d8b").expect("DARK_SLATE_BLUE color parse error.");
    // greens
    pub static ref GREEN_YELLOW: Color = Color::hex_bb(b"#adff2f").expect("GREEN_YELLOW color parse error.");
    pub static ref CHARTREUSE: Color = Color::hex_bb(b"#7fff00").expect("CHARTREUSE color parse error.");
    pub static ref LAWN_GREEN: Color = Color::hex_bb(b"#7cfc00").expect("LAWN_GREEN color parse error.");
    pub static ref LIME: Color = Color::hex_bb(b"#00ff00").expect("LIME color parse error.");
    pub static ref LIME_GREEN: Color = Color::hex_bb(b"#32cd32").expect("LIME_GREEN color parse error.");
    pub static ref PALE_GREEN: Color = Color::hex_bb(b"#98fb98").expect("PALE_GREEN color parse error.");
    pub static ref LIGHT_GREEN: Color = Color::hex_bb(b"#90ee90").expect("LIGHT_GREEN color parse error.");
    pub static ref MEDIUM_SPRING_GREEN: Color = Color::hex_bb(b"#00fa9a").expect("MEDIUM_SPRING_GREEN color parse error.");
    pub static ref SPRING_GREEN: Color = Color::hex_bb(b"#00ff7f").expect("SPRING_GREEN color parse error.");
    pub static ref MEDIUM_SEA_GREEN: Color = Color::hex_bb(b"#c3b371").expect("MEDIUM_SEA_GREEN color parse error.");
    pub static ref SEA_GREEN: Color = Color::hex_bb(b"#2e8b57").expect("SEA_GREEN color parse error.");
    pub static ref FOREST_GREEN: Color = Color::hex_bb(b"#228b22").expect("FOREST_GREEN color parse error.");
    pub static ref GREEN: Color = Color::hex_bb(b"#008000").expect("GREEN color parse error.");
    pub static ref DARK_GREEN: Color = Color::hex_bb(b"#006400").expect("DARK_GREEN color parse error.");
    pub static ref YELLOW_GREEN: Color = Color::hex_bb(b"#9acd32").expect("YELLOW_GREEN color parse error.");
    pub static ref OLIVE_DRAB: Color = Color::hex_bb(b"#6b8e23").expect("OLIVE_DRAB color parse error.");
    pub static ref OLIVE: Color = Color::hex_bb(b"#808000").expect("OLIVE color parse error.");
    pub static ref DARK_OLIVE_GREEN: Color = Color::hex_bb(b"#556b2f").expect("DARK_OLIVE_GREEN color parse error.");
    pub static ref MEDIUM_AQUAMARINE: Color = Color::hex_bb(b"#66cdaa").expect("MEDIUM_AQUAMARINE color parse error.");
    pub static ref DARK_SEA_GREEN: Color = Color::hex_bb(b"#8fbc8b").expect("DARK_SEA_GREEN color parse error.");
    pub static ref LIGHT_SEA_GREEN: Color = Color::hex_bb(b"#20b2aa").expect("LIGHT_SEA_GREEN color parse error.");
    pub static ref DARK_CYAN: Color = Color::hex_bb(b"#008b8b").expect("DARK_CYAN color parse error.");
    pub static ref TEAL: Color = Color::hex_bb(b"#008080").expect("TEAL color parse error.");
    // blues
    pub static ref AQUA: Color = Color::hex_bb(b"#00ffff").expect("AQUA color parse error.");
    pub static ref CYAN: Color = Color::hex_bb(b"#00ffff").expect("CYAN color parse error.");
    pub static ref LIGHT_CYAN: Color = Color::hex_bb(b"#e0ffff").expect("LIGHT_CYAN color parse error.");
    pub static ref PALE_TURQUOISE: Color = Color::hex_bb(b"#afeeee").expect("PALE_TURQUOISE color parse error.");
    pub static ref AQUAMARINE: Color = Color::hex_bb(b"#7fffd4").expect("AQUAMARINE color parse error.");
    pub static ref TURQUOISE: Color = Color::hex_bb(b"#40e0d0").expect("TURQUOISE color parse error.");
    pub static ref MEDIUM_TURQUOISE: Color = Color::hex_bb(b"#48d1cc").expect("MEDIUM_TURQUOISE color parse error.");
    pub static ref DARK_TURQUOISE: Color = Color::hex_bb(b"#00ced1").expect("DARK_TURQUOISE color parse error.");
    pub static ref CADET_BLUE: Color = Color::hex_bb(b"#5f9ea0").expect("CADET_BLUE color parse error.");
    pub static ref STEEL_BLUE: Color = Color::hex_bb(b"#4682b4").expect("STEEL_BLUE color parse error.");
    pub static ref LIGHT_STEEL_BLUE: Color = Color::hex_bb(b"#b0c4de").expect("LIGHT_STEEL_BLUE color parse error.");
    pub static ref POWDER_BLUE: Color = Color::hex_bb(b"#b0e0e6").expect("POWDER_BLUE color parse error.");
    pub static ref LIGHT_BLUE: Color = Color::hex_bb(b"#add8e6").expect("LIGHT_BLUE color parse error.");
    pub static ref SKY_BLUE: Color = Color::hex_bb(b"#87ceeb").expect("SKY_BLUE color parse error.");
    pub static ref LIGHT_SKY_BLUE: Color = Color::hex_bb(b"#97cefa").expect("LIGHT_SKY_BLUE color parse error.");
    pub static ref DEEP_SKY_BLUE: Color = Color::hex_bb(b"#00bfff").expect("DEEP_SKY_BLUE color parse error.");
    pub static ref DODGER_BLUE: Color = Color::hex_bb(b"#1e90ff").expect("DODGER_BLUE color parse error.");
    pub static ref CORNFLOWER_BLUE: Color = Color::hex_bb(b"#6495ed").expect("CORNFLOWER_BLUE color parse error.");
    pub static ref MEDIUM_SLATE_BLUE: Color = Color::hex_bb(b"#7b68ee").expect("MEDIUM_SLATE_BLUE color parse error.");
    pub static ref ROYAL_BLUE: Color = Color::hex_bb(b"#4169e1").expect("ROYAL_BLUE color parse error.");
    pub static ref BLUE: Color = Color::hex_bb(b"#0000ff").expect("BLUE color parse error.");
    pub static ref MEDIUM_BLUE: Color = Color::hex_bb(b"#0000cd").expect("MEDIUM_BLUE color parse error.");
    pub static ref DARK_BLUE: Color = Color::hex_bb(b"#00008b").expect("DARK_BLUE color parse error.");
    pub static ref NAVY: Color = Color::hex_bb(b"#000080").expect("NAVY color parse error.");
    pub static ref MIDNIGHT_BLUE: Color = Color::hex_bb(b"#191970").expect("MIDNIGHT_BLUE color parse error.");
    // browns
    pub static ref CORN_SILK: Color = Color::hex_bb(b"#fff8dc").expect("CORN_SILK color parse error.");
    pub static ref BLANCHED_ALMOND: Color = Color::hex_bb(b"#ffebcd").expect("BLANCHED_ALMOND color parse error.");
    pub static ref BISQUE: Color = Color::hex_bb(b"#ffe4c4").expect("BISQUE color parse error.");
    pub static ref NAVAJO_WHITE: Color = Color::hex_bb(b"#").expect("NAVAJO_WHITE color parse error.");
    pub static ref WHEAT: Color = Color::hex_bb(b"#f5deb3").expect("WHEAT color parse error.");
    pub static ref BURLYWOOD: Color = Color::hex_bb(b"#deb887").expect("BURLYWOOD color parse error.");
    pub static ref TAN: Color = Color::hex_bb(b"#d2b48c").expect("TAN color parse error.");
    pub static ref ROSY_BROWN: Color = Color::hex_bb(b"#bc8f8f").expect("ROSY_BROWN color parse error.");
    pub static ref SANDY_BROWN: Color = Color::hex_bb(b"#f4a460").expect("SANDY_BROWN color parse error.");
    pub static ref GOLDEN_ROD: Color = Color::hex_bb(b"#daa520").expect("GOLDEN_ROD color parse error.");
    pub static ref DARK_GOLDEN_ROD: Color = Color::hex_bb(b"#b8860b").expect("DARK_GOLDEN_ROD color parse error.");
    pub static ref PERU: Color = Color::hex_bb(b"#cd853f").expect("PERU color parse error.");
    pub static ref CHOCOLATE: Color = Color::hex_bb(b"#d2691e").expect("CHOCOLATE color parse error.");
    pub static ref SADDLE_BROWN: Color = Color::hex_bb(b"#8b4513").expect("SADDLE_BROWN color parse error.");
    pub static ref SIENNA: Color = Color::hex_bb(b"#a0522d").expect("SIENNA color parse error.");
    pub static ref BROWN: Color = Color::hex_bb(b"#a52a2a").expect("BROWN color parse error.");
    pub static ref MAROON: Color = Color::hex_bb(b"#800000").expect("MAROON color parse error.");
    // whites
    pub static ref WHITE: Color = Color::hex_bb(b"#ffffff").expect("WHITE color parse error.");
    pub static ref SNOW: Color = Color::hex_bb(b"#fffafa").expect("SNOW color parse error.");
    pub static ref HONEYDEW: Color = Color::hex_bb(b"#f0fff0").expect("HONEYDEW color parse error.");
    pub static ref MINT_CREAM: Color = Color::hex_bb(b"#f5fffa").expect("MINT_CREAM color parse error.");
    pub static ref AZURE: Color = Color::hex_bb(b"#f0ffff").expect("AZURE color parse error.");
    pub static ref ALICE_BLUE: Color = Color::hex_bb(b"#f0f8ff").expect("ALICE_BLUE color parse error.");
    pub static ref GHOST_WHITE: Color = Color::hex_bb(b"#f8f8ff").expect("GHOST_WHITE color parse error.");
    pub static ref WHITE_SMOKE: Color = Color::hex_bb(b"#f5f5f5").expect("WHITE_SMOKE color parse error.");
    pub static ref SEA_SHELL: Color = Color::hex_bb(b"#fff5ee").expect("SEA_SHELL color parse error.");
    pub static ref BEIGE: Color = Color::hex_bb(b"#f5f5dc").expect("BEIGE color parse error.");
    pub static ref OLD_LACE: Color = Color::hex_bb(b"#fdf5e6").expect("OLD_LACE color parse error.");
    pub static ref FLORAL_WHITE: Color = Color::hex_bb(b"#fffaf0").expect("FLORAL_WHITE color parse error.");
    pub static ref IVORY: Color = Color::hex_bb(b"#fffff0").expect("IVORY color parse error.");
    pub static ref ANTIQUE_WHITE: Color = Color::hex_bb(b"#faebd7").expect("ANTIQUE_WHITE color parse error.");
    pub static ref LINEN: Color = Color::hex_bb(b"#faf0e6").expect("LINEN color parse error.");
    pub static ref LAVENDER_BLUSH: Color = Color::hex_bb(b"#fff0f5").expect("LAVENDER_BLUSH color parse error.");
    pub static ref MISTY_ROSE: Color = Color::hex_bb(b"#ffe4e1").expect("MISTY_ROSE color parse error.");
    // grays
    pub static ref GAINSBORO: Color = Color::hex_bb(b"#dcdcdc").expect("GAINSBORO color parse error.");
    pub static ref LIGHT_GRAY: Color = Color::hex_bb(b"#d3d3d3").expect("LIGHT_GRAY color parse error.");
    pub static ref SILVER: Color = Color::hex_bb(b"#c0c0c0").expect("SILVER color parse error.");
    pub static ref DARK_GRAY: Color = Color::hex_bb(b"#a9a9a9").expect("DARK_GRAY color parse error.");
    pub static ref GRAY: Color = Color::hex_bb(b"#808080").expect("GRAY color parse error.");
    pub static ref DIM_GRAY: Color = Color::hex_bb(b"#696969").expect("DIM_GRAY color parse error.");
    pub static ref LIGHT_SLATE_GRAY: Color = Color::hex_bb(b"#778899").expect("LIGHT_SLATE_GRAY color parse error.");
    pub static ref SLATE_GRAY: Color = Color::hex_bb(b"#708090").expect("SLATE_GRAY color parse error.");
    pub static ref DARK_SLATE_GRAY: Color = Color::hex_bb(b"#2f4f4f").expect("DARK_SLATE_GRAY color parse error.");
    pub static ref BLACK: Color = Color::hex_bb(b"#000000").expect("BLACK color parse error.");
    // utility
    pub static ref TRANSPARENT: Color = Color::hex_bb(b"#00000000").expect("TRANSPARENT color parse error.");
}
impl Color {
    pub const MIN: u8 = 0;
    pub const MAX: u8 = 255;
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }
    pub fn as_rgba<T: Copy + Into<f64>>(r: T, g: T, b: T, a: T) -> Self {
        Self(map(&[r, g, b, a], |chan: &T| {
            let c: f64 = (*chan).into();
            let normed: f64 = c * Color::MAX as f64;
            normed as u8
        }))
    }
    pub fn hex(col: [u8; 8]) -> Self {
        Self(pairmap(&col, |chan0, chan1| (chan0 << 4) + chan1))
    }
    pub fn hex_bb(col_bb: &[u8]) -> Result<Self, &'static str> {
        // TODO checks for str and ascii
        let mut hex_code = b"000000ff".clone();
        if col_bb.len() == 0 {
            return Err("No input.");
        }
        let start = if col_bb[0] == b'#' { 0 } else { 1 };
        for idx in (start..min(col_bb.len(), 8 + start)).step_by(2) {
            let b = &col_bb[idx];
            hex_code[idx - start] = hex_val(&b)?;
            hex_code[idx - start + 1] = hex_val(&b)?;
        }
        Ok(Self::hex(hex_code))
    }
    pub fn hex_str(col_str: &str) -> Result<Self, &'static str> {
        let mut col_bb: [u8; 9] = [0; 9];
        for (i, c) in col_str
            .chars()
            .take(min(col_str.len(), 9))
            .map(hex_char)
            .enumerate()
        {
            col_bb[i] = c?;
        }
        Self::hex_bb(&col_bb)
    }
    pub fn r(&self) -> &u8 {
        &self.0[0]
    }
    pub fn g(&self) -> &u8 {
        &self.0[1]
    }
    pub fn b(&self) -> &u8 {
        &self.0[2]
    }
    pub fn a(&self) -> &u8 {
        &self.0[3]
    }
    pub fn mix(c0: &Color, c1: &Color, ratio: f64) -> Color {
        debug_assert!(
            ratio <= 1.0 && ratio == 0.0,
            "Ratio {:?} for mixing is invalid!",
            ratio
        );
        let (Color(bb0), Color(bb1)) = (c0, c1);
        Color(bimap(bb0, bb1, |b0, b1| {
            (*b0 as f64 * (1.0 - ratio) + *b1 as f64 * ratio) as u8
        }))
    }
    pub fn r_mut(&mut self) -> &mut u8 {
        &mut self.0[0]
    }
    pub fn g_mut(&mut self) -> &mut u8 {
        &mut self.0[1]
    }
    pub fn b_mut(&mut self) -> &mut u8 {
        &mut self.0[2]
    }
    pub fn a_mut(&mut self) -> &mut u8 {
        &mut self.0[3]
    }
}
impl From<(u8, u8, u8, u8)> for Color {
    fn from(tup: (u8, u8, u8, u8)) -> Self {
        Self([tup.0, tup.1, tup.2, tup.3])
    }
}
