//! Shared candidate geometry. Rectangles use physical pixels after DPI scaling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoxRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
impl BoxRect {
    pub fn contains(self, x: i32, y: i32) -> bool {
        self.width > 0
            && self.height > 0
            && x >= self.x
            && y >= self.y
            && x < self.x.saturating_add(self.width)
            && y < self.y.saturating_add(self.height)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub column: i32,
    pub row: i32,
    pub columns: usize,
    pub count: usize,
    pub extra: usize,
    pub padding: i32,
    pub gap: i32,
    pub header: i32,
    pub compact: bool,
    pub narrow: bool,
    width: i32,
    chinese_font: i32,
    english_font: i32,
    title: BoxRect,
    mode: BoxRect,
    controls: [BoxRect; 5],
    chinese: [BoxRect; 5],
    english: [BoxRect; 5],
    body_top: i32,
    body_bottom: i32,
    sense_height: i32,
    footer_height: i32,
    viewport_height: i32,
    scroll: i32,
}
impl Layout {
    pub fn new(count: usize, extra: usize, font: u32, dpi: i32, available: i32) -> Self {
        Self::with_lengths(&[2; 5][..count.min(5)], extra, font, dpi, available)
    }
    pub fn with_lengths(
        lengths: &[usize],
        extra: usize,
        font: u32,
        dpi: i32,
        available: i32,
    ) -> Self {
        let dpi = dpi.clamp(96, 768);
        let scale = |value: i32| ((i64::from(value) * i64::from(dpi) + 48) / 96) as i32;
        let count = lengths.len().min(5);
        let lengths = &lengths[..count];
        let compact = lengths.iter().all(|length| *length <= 8);
        let chinese_font = scale(((font.clamp(12, 32) * 15 + 9) / 18) as i32);
        let english_font = scale(((font.clamp(12, 32) * 13 + 9) / 18) as i32);
        let padding = scale(12);
        let gap = scale(4);
        let row = scale(44);
        let estimated_width: i32 = lengths
            .iter()
            .map(|length| {
                let glyphs = i32::try_from(length.saturating_add(2)).unwrap_or(i32::MAX);
                let title_width = chinese_font.saturating_mul(glyphs);
                title_width
                    .saturating_add(padding * 2)
                    .clamp(row, scale(140))
            })
            .sum();
        let wanted = if compact && count < 5 {
            (estimated_width + gap * count.saturating_sub(1) as i32 + padding * 2).max(scale(400))
        } else {
            scale(560)
        };
        let width = wanted.min(scale(560)).min(available.max(row));
        let body_width = (width - padding * 2).max(row);
        let narrow = width < scale(500);
        let control_width = row * 4 + scale(28);
        let mode_width = scale(22);
        // A second header row retains native-size actions on very narrow work areas.
        let stacked_header = body_width < control_width + mode_width + row;
        let header = if stacked_header { row * 2 } else { row };
        let control_y = padding + if stacked_header { row } else { 0 };
        let control_x = (width - padding - control_width).max(padding);
        let mut controls = [BoxRect::default(); 5];
        let mut cursor = control_x;
        for (index, item_width) in [row, scale(28), row, row, row].into_iter().enumerate() {
            controls[index] = BoxRect {
                x: cursor,
                y: control_y,
                width: item_width,
                height: row,
            };
            cursor += item_width;
        }
        let mode_x = if stacked_header {
            width - padding - mode_width
        } else {
            control_x - mode_width
        };
        let mode = BoxRect {
            x: mode_x,
            y: padding,
            width: mode_width,
            height: row,
        };
        let title = BoxRect {
            x: padding,
            y: padding,
            width: (mode_x - padding - gap).max(0),
            height: row,
        };
        let body_top = padding + header + gap * 2 + scale(1);
        let mut chinese = [BoxRect::default(); 5];
        let mut english = [BoxRect::default(); 5];
        let columns = if compact {
            ((body_width + gap) / (row + gap)).max(1) as usize
        } else {
            1
        }
        .min(count.max(1));
        let column = if compact {
            (body_width - gap * (columns - 1) as i32) / columns as i32
        } else {
            body_width
        };
        // Full-width glyphs conservatively estimate wrapping. Include the local
        // number and button padding, with no line cap or candidate truncation.
        let text_height = |length: usize, width: i32| {
            let chars_per_line =
                ((width - padding * 2).max(chinese_font) / chinese_font).max(1) as usize;
            let lines = length.saturating_add(2).div_ceil(chars_per_line).max(1);
            let line_height = (chinese_font * 7 + 4) / 5;
            i32::try_from(lines)
                .unwrap_or(i32::MAX / line_height)
                .saturating_mul(line_height)
                .saturating_add(scale(16))
                .max(row)
        };
        let mut y = body_top;
        if compact {
            for group in 0..count.div_ceil(columns) {
                let start = group * columns;
                let end = (start + columns).min(count);
                let chinese_height = lengths[start..end]
                    .iter()
                    .map(|length| text_height(*length, column))
                    .max()
                    .unwrap_or(row);
                for slot in start..end {
                    let x = padding + (slot - start) as i32 * (column + gap);
                    chinese[slot] = BoxRect {
                        x,
                        y,
                        width: column,
                        height: chinese_height,
                    };
                    english[slot] = BoxRect {
                        x,
                        y: y + chinese_height,
                        width: column,
                        height: row,
                    };
                }
                y += chinese_height + row + gap;
            }
        } else {
            for (slot, length) in lengths.iter().enumerate() {
                let chinese_width = if narrow { body_width } else { scale(128) };
                let chinese_height = text_height(*length, chinese_width);
                chinese[slot] = BoxRect {
                    x: padding,
                    y,
                    width: chinese_width,
                    height: chinese_height,
                };
                english[slot] = if narrow {
                    BoxRect {
                        x: padding,
                        y: y + chinese_height,
                        width: body_width,
                        height: row,
                    }
                } else {
                    BoxRect {
                        x: padding + chinese_width + scale(8),
                        y,
                        width: body_width - chinese_width - scale(8),
                        height: chinese_height,
                    }
                };
                y += chinese_height + if narrow { row } else { 0 } + gap;
            }
        }
        let body_bottom = if count == 0 {
            body_top + row * 2
        } else {
            y - gap
        };
        Self {
            column,
            row,
            columns,
            count,
            extra,
            padding,
            gap,
            header,
            compact,
            narrow,
            width,
            chinese_font,
            english_font,
            title,
            mode,
            controls,
            chinese,
            english,
            body_top,
            body_bottom,
            sense_height: scale(88),
            footer_height: scale(24),
            viewport_height: i32::MAX,
            scroll: 0,
        }
    }
    pub fn width(self) -> i32 {
        self.width
    }
    pub fn height(self) -> i32 {
        self.full_height().min(self.viewport_height)
    }
    pub fn full_height(self) -> i32 {
        if self.extra == 0 {
            self.body_bottom + self.padding + self.scroll
        } else {
            self.footer_rect().y + self.footer_height + self.padding + self.scroll
        }
    }
    pub fn with_viewport(mut self, available_height: i32, offset: i32) -> Self {
        self.viewport_height = available_height.max(self.body_top + self.row + self.padding);
        self.scroll = offset.clamp(0, self.scroll_max());
        for r in self.chinese.iter_mut().chain(self.english.iter_mut()) {
            r.y -= self.scroll;
        }
        self.body_bottom -= self.scroll;
        self
    }
    pub fn scroll_offset(self) -> i32 {
        self.scroll
    }
    pub fn scroll_max(self) -> i32 {
        (self.full_height() - self.height()).max(0)
    }
    pub fn visible(self, r: BoxRect) -> BoxRect {
        let top = r.y.max(self.body_top);
        let bottom = (r.y + r.height).min(self.height() - self.padding);
        BoxRect {
            y: top,
            height: (bottom - top).max(0),
            ..r
        }
    }
    pub fn groups(self) -> usize {
        self.count.max(1).div_ceil(self.columns)
    }
    /// Font sizes are physical pixels, ready for the native font constructor.
    pub fn chinese_font(self) -> i32 {
        self.chinese_font
    }
    pub fn english_font(self) -> i32 {
        self.english_font
    }
    /// 0 previous, 1 page label, 2 next, 3 details, 4 settings.
    pub fn header_item(self, index: usize) -> BoxRect {
        self.controls.get(index).copied().unwrap_or_default()
    }
    pub fn title_rect(self) -> BoxRect {
        self.title
    }
    pub fn mode_rect(self) -> BoxRect {
        self.mode
    }
    pub fn body_rect(self) -> BoxRect {
        BoxRect {
            x: self.padding,
            y: self.body_top,
            width: self.width - self.padding * 2,
            height: (self.body_bottom - self.body_top).max(0),
        }
    }
    pub fn candidate_rect(self, slot: usize, english: bool) -> BoxRect {
        if slot >= self.count {
            return BoxRect::default();
        }
        if english {
            self.english[slot]
        } else {
            self.chinese[slot]
        }
    }
    pub fn position(self, slot: usize) -> (i32, i32) {
        let rect = self.candidate_rect(slot, false);
        (rect.x, rect.y)
    }
    pub fn hit(self, x: i32, y: i32) -> Option<(usize, bool)> {
        if y < self.body_top || y >= self.height() - self.padding {
            return None;
        }
        for slot in 0..self.count {
            for english in [false, true] {
                if self.candidate_rect(slot, english).contains(x, y) {
                    return Some((slot, english));
                }
            }
        }
        None
    }
    pub fn details_title_rect(self) -> BoxRect {
        BoxRect {
            x: self.padding,
            y: self.body_bottom + self.gap,
            width: self.width - self.padding * 2,
            height: self.row,
        }
    }
    pub fn senses_y(self) -> i32 {
        let title = self.details_title_rect();
        title.y + title.height + self.gap
    }
    pub fn sense_rect(self, row: usize) -> BoxRect {
        if row >= self.extra {
            return BoxRect::default();
        }
        BoxRect {
            x: self.padding,
            y: self.senses_y() + row as i32 * self.sense_height,
            width: self.width - self.padding * 2,
            height: self.sense_height,
        }
    }
    pub fn visible_senses(self) -> std::ops::Range<usize> {
        let top = (self.body_top - self.senses_y()).max(0);
        let bottom = (self.height() - self.padding - self.senses_y()).max(0);
        let start = (top / self.sense_height) as usize;
        let end = ((bottom + self.sense_height - 1) / self.sense_height) as usize;
        start.min(self.extra)..end.min(self.extra)
    }
    pub fn sense_hit(self, x: i32, y: i32) -> Option<usize> {
        if y < self.body_top || y >= self.height() - self.padding || y < self.senses_y() {
            return None;
        }
        let row = ((y - self.senses_y()) / self.sense_height) as usize;
        (row < self.extra && self.visible(self.sense_rect(row)).contains(x, y)).then_some(row)
    }
    pub fn footer_rect(self) -> BoxRect {
        BoxRect {
            x: self.padding,
            y: self.senses_y() + self.extra as i32 * self.sense_height + self.gap,
            width: self.width - self.padding * 2,
            height: self.footer_height,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compact_geometry_has_shared_targets_and_mac_spacing_at_each_dpi() {
        for font in [16, 18, 20, 22] {
            for dpi in [96, 144, 192] {
                for width in [320, 440, 560] {
                    let available = width * dpi / 96;
                    let layout = Layout::new(5, 6, font, dpi, available);
                    assert!(layout.width() <= available);
                    assert_eq!(layout.padding, 12 * dpi / 96);
                    assert_eq!(layout.gap, 4 * dpi / 96);
                    for slot in 0..5 {
                        for english in [false, true] {
                            let rect = layout.candidate_rect(slot, english);
                            assert!(rect.width >= 44 * dpi / 96);
                            assert!(rect.height >= 44 * dpi / 96);
                            assert_eq!(layout.hit(rect.x + 2, rect.y + 2), Some((slot, english)));
                            assert!(rect.x + rect.width <= layout.width() - layout.padding);
                        }
                    }
                    assert_eq!(layout.hit(-1, layout.row), None);
                    assert_eq!(layout.hit(layout.padding, layout.senses_y()), None);
                    for index in [0, 2, 3, 4] {
                        let rect = layout.header_item(index);
                        assert_eq!(rect.width, layout.row);
                        assert_eq!(rect.height, layout.row);
                    }
                    assert_eq!(layout.sense_rect(5).height, 88 * dpi / 96);
                    assert_eq!(layout.footer_rect().height, 24 * dpi / 96);
                }
            }
        }
    }
    #[test]
    fn long_chinese_wraps_without_a_line_cap_and_stacks_on_narrow_widths() {
        for dpi in [96, 144, 192] {
            let wide = Layout::with_lengths(&[9, 240], 0, 18, dpi, 560 * dpi / 96);
            assert!(!wide.compact && !wide.narrow);
            assert_eq!(wide.columns, 1);
            assert!(wide.candidate_rect(1, false).height > wide.row * 8);
            assert_eq!(
                wide.candidate_rect(0, false).y,
                wide.candidate_rect(0, true).y
            );
            let narrow = Layout::with_lengths(&[9, 32], 2, 22, dpi, 440 * dpi / 96);
            assert!(narrow.narrow);
            for slot in 0..2 {
                let chinese = narrow.candidate_rect(slot, false);
                let english = narrow.candidate_rect(slot, true);
                assert_eq!(english.y, chinese.y + chinese.height);
                assert_eq!(english.width, chinese.width);
                assert_eq!(narrow.hit(english.x + 1, english.y + 1), Some((slot, true)));
            }
            assert!(narrow.details_title_rect().y >= narrow.candidate_rect(1, true).y + narrow.row);
        }
    }
    #[test]
    fn compact_height_and_font_scale_match_the_short_candidate_contract() {
        let layout = Layout::new(5, 0, 18, 96, 560);
        assert_eq!(layout.chinese_font(), 15);
        assert_eq!(layout.english_font(), 13);
        assert!(layout.width() >= 400 && layout.width() <= 560);
        assert_eq!(layout.width(), 560);
        assert!(layout.height() <= 190);
        let empty = Layout::new(0, 0, 18, 96, 560);
        assert_eq!(empty.width(), 400);
        assert_eq!(empty.body_rect().height, 88);
        assert_eq!(empty.hit(20, empty.body_rect().y + 2), None);
        assert_eq!(layout.candidate_rect(5, false), BoxRect::default());
        assert_eq!(layout.sense_rect(0), BoxRect::default());
    }
    #[test]
    fn large_translation_lists_only_visit_visible_rows_and_keep_absolute_indices() {
        for dpi in [96, 144, 192] {
            let full = Layout::new(5, 100_000, 18, dpi, 560 * dpi / 96);
            for offset in [0, full.full_height() / 2, i32::MAX] {
                let l = full.with_viewport(400 * dpi / 96, offset);
                assert!(l.visible_senses().len() <= 6);
                for row in l.visible_senses() {
                    let r = l.visible(l.sense_rect(row));
                    assert!(r.height > 0);
                    assert_eq!(l.sense_hit(r.x + 2, r.y + r.height / 2), Some(row));
                }
                assert_eq!(l.sense_hit(l.padding, l.header_item(0).y), None);
            }
            let last = full.with_viewport(400 * dpi / 96, i32::MAX);
            assert_eq!(last.visible_senses().end, 100_000);
        }
    }
}
