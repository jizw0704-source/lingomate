//! Shared geometry for painting and hit testing. Sizes are physical pixels after DPI scaling.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub column: i32,
    pub row: i32,
    pub columns: usize,
    pub count: usize,
    pub extra: usize,
    minimum_width: i32,
}
impl Layout {
    pub fn new(count: usize, extra: usize, font: u32, dpi: i32, available: i32) -> Self {
        let column = ((128 * font as i32 / 18) * dpi + 48) / 96;
        let row = (44 * dpi + 48) / 96;
        let columns = (available / column).max(1) as usize;
        Self {
            column,
            row,
            columns: columns.min(count.max(2)).min(5),
            count,
            extra,
            minimum_width: available.min(row * 5 + (160 * dpi + 48) / 96),
        }
    }
    pub fn width(self) -> i32 {
        (self.columns as i32 * self.column)
            .max(self.row * 5)
            .max(self.minimum_width)
    }
    pub fn groups(self) -> usize {
        self.count.max(1).div_ceil(self.columns)
    }
    pub fn senses_y(self) -> i32 {
        self.row + self.groups() as i32 * self.row * 2
    }
    pub fn height(self) -> i32 {
        self.senses_y() + self.extra as i32 * self.row
    }
    pub fn position(self, slot: usize) -> (i32, i32) {
        (
            (slot % self.columns) as i32 * self.column,
            self.row + (slot / self.columns) as i32 * self.row * 2,
        )
    }
    pub fn hit(self, x: i32, y: i32) -> Option<(usize, bool)> {
        if x < 0 || y < self.row || x >= self.columns as i32 * self.column || y >= self.senses_y() {
            return None;
        }
        let offset = y - self.row;
        let slot = (offset / (self.row * 2)) as usize * self.columns + (x / self.column) as usize;
        (slot < self.count).then_some((slot, offset % (self.row * 2) >= self.row))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_fonts_dpis_narrow_layouts_keep_targets_and_exact_indices() {
        for font in [16, 18, 20, 22] {
            for dpi in [96, 144, 192] {
                for width in [320, 640, 1280] {
                    let l = Layout::new(5, 6, font, dpi, width);
                    assert!(l.row >= 44 * dpi / 96);
                    for slot in 0..5 {
                        let (x, y) = l.position(slot);
                        assert_eq!(l.hit(x + 4, y + 4), Some((slot, false)));
                        assert_eq!(l.hit(x + 4, y + l.row + 4), Some((slot, true)));
                    }
                    assert_eq!(l.hit(-1, l.row), None);
                    assert_eq!(l.hit(0, l.senses_y()), None);
                }
            }
        }
    }
}
