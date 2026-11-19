//! 编号书签 0–9：与视图无关的书签集合与「编辑前后行表比对」的平移规则。
//! 文本比较的锚点为 `(侧, 行号)`，文本合并的锚点为 `(窗格, 对齐行)`。
use bc_core::line::LineList;

/// 书签个数（编号 0–9）。
pub const COUNT: usize = 10;

/// 一个视图的书签：编号 → 锚点。纯数据，规则见 [`Bookmarks::toggle`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmarks<A> {
    slots: [Option<A>; COUNT],
}

impl<A> Default for Bookmarks<A> {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
        }
    }
}

/// [`Bookmarks::toggle`] 的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggled {
    Placed,
    Removed,
}

impl<A: Copy + PartialEq> Bookmarks<A> {
    /// 书签 `n` 已在 `at` 上则移除；否则放到 `at`（`n` 原在别处时移过来），
    /// `at` 上原有的其他编号被取代（一行最多一个书签）。
    pub fn toggle(&mut self, n: usize, at: A) -> Toggled {
        if self.slots[n] == Some(at) {
            self.slots[n] = None;
            return Toggled::Removed;
        }
        for slot in &mut self.slots {
            if *slot == Some(at) {
                *slot = None;
            }
        }
        self.slots[n] = Some(at);
        Toggled::Placed
    }
    pub fn get(&self, n: usize) -> Option<A> {
        self.slots.get(n).copied().flatten()
    }
    pub fn clear(&mut self) {
        self.slots = [None; COUNT];
    }
    /// 锚点上的书签编号（编辑合并到同一行时可能有多个，取最小编号）。
    pub fn at(&self, anchor: A) -> Option<usize> {
        self.slots.iter().position(|s| *s == Some(anchor))
    }
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }
    /// 已设置的书签（编号, 锚点），按编号升序。
    pub fn iter(&self) -> impl Iterator<Item = (usize, A)> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(n, s)| s.map(|a| (n, a)))
    }
    /// 改写每个锚点（交换两侧、按编辑平移）。
    pub fn map(&mut self, mut f: impl FnMut(A) -> A) {
        for slot in self.slots.iter_mut().flatten() {
            *slot = f(*slot);
        }
    }
    /// 只保留满足条件的书签（换文件时清除一侧）。
    pub fn retain(&mut self, mut keep: impl FnMut(A) -> bool) {
        for slot in &mut self.slots {
            if slot.is_some_and(|a| !keep(a)) {
                *slot = None;
            }
        }
    }
}

/// 可按行比对的行表（[`LineList`]；单测用字符串切片）。
pub trait Lines {
    fn count(&self) -> usize;
    fn bytes(&self, index: usize) -> &[u8];
}

impl Lines for LineList {
    fn count(&self) -> usize {
        self.len()
    }
    fn bytes(&self, index: usize) -> &[u8] {
        self.line_bytes(index).unwrap_or_default()
    }
}

impl Lines for [&str] {
    fn count(&self) -> usize {
        self.len()
    }
    fn bytes(&self, index: usize) -> &[u8] {
        self[index].as_bytes()
    }
}

/// 编辑前后行表的一处变化：相同前缀 `prefix` 行、相同后缀 `suffix` 行。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Change {
    prefix: usize,
    suffix: usize,
    old_len: usize,
    new_len: usize,
}

impl Change {
    /// 行数不变时为 `None`（原位修改，不平移）。
    pub fn between<L: Lines + ?Sized>(old: &L, new: &L) -> Option<Self> {
        let (old_len, new_len) = (old.count(), new.count());
        if old_len == new_len {
            return None;
        }
        let shorter = old_len.min(new_len);
        let prefix = (0..shorter)
            .find(|&i| old.bytes(i) != new.bytes(i))
            .unwrap_or(shorter);
        let suffix = (0..shorter - prefix)
            .find(|&i| old.bytes(old_len - 1 - i) != new.bytes(new_len - 1 - i))
            .unwrap_or(shorter - prefix);
        Some(Self {
            prefix,
            suffix,
            old_len,
            new_len,
        })
    }
    /// 编辑前第 `line` 行在编辑后的位置：变化区间之前不动，之后按行数增量平移；
    /// 区间内保持相对区间起点的偏移（钳制到新区间末行），新区间为空（纯删除）时落到区间起点。
    pub fn apply(&self, line: usize) -> usize {
        let old_end = self.old_len - self.suffix;
        let new_end = self.new_len - self.suffix;
        let moved = if line < self.prefix {
            line
        } else if line >= old_end {
            line + self.new_len - self.old_len
        } else if new_end > self.prefix {
            line.min(new_end - 1)
        } else {
            self.prefix
        };
        clamp(moved, self.new_len)
    }
}

/// 把行号钳制到 `len` 行的文件内（空文件为第 0 行）。
pub fn clamp(line: usize, len: usize) -> usize {
    line.min(len.saturating_sub(1))
}

/// 编辑前第 `line` 行在编辑后的行号（设计决定 2）。
pub fn shift<L: Lines + ?Sized>(old: &L, new: &L, line: usize) -> usize {
    match Change::between(old, new) {
        Some(change) => change.apply(line),
        None => clamp(line, new.count()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Marks = Bookmarks<(usize, usize)>;
    const LEFT: usize = 0;

    #[test]
    fn toggle_places_a_bookmark_on_the_current_line() {
        let mut b = Marks::default();
        assert!(b.is_empty());
        assert_eq!(b.toggle(3, (LEFT, 119)), Toggled::Placed);
        assert_eq!(b.get(3), Some((LEFT, 119)));
        assert_eq!(b.at((LEFT, 119)), Some(3));
        assert!(!b.is_empty());
    }

    #[test]
    fn toggling_the_same_number_on_its_line_removes_it() {
        let mut b = Marks::default();
        b.toggle(3, (LEFT, 119));
        assert_eq!(b.toggle(3, (LEFT, 119)), Toggled::Removed);
        assert_eq!(b.get(3), None);
        assert_eq!(b.at((LEFT, 119)), None);
        assert!(b.is_empty());
    }

    #[test]
    fn toggling_a_number_elsewhere_moves_it() {
        let mut b = Marks::default();
        b.toggle(3, (LEFT, 119));
        assert_eq!(b.toggle(3, (LEFT, 399)), Toggled::Placed);
        assert_eq!(b.get(3), Some((LEFT, 399)));
        assert_eq!(b.at((LEFT, 119)), None);
    }

    #[test]
    fn another_number_on_the_same_line_replaces_the_old_one() {
        let mut b = Marks::default();
        b.toggle(3, (LEFT, 119));
        b.toggle(5, (LEFT, 119));
        assert_eq!(b.get(5), Some((LEFT, 119)));
        assert_eq!(b.get(3), None);
        assert_eq!(b.iter().collect::<Vec<_>>(), vec![(5, (LEFT, 119))]);
    }

    #[test]
    fn the_same_line_number_on_the_other_side_is_a_different_anchor() {
        let mut b = Marks::default();
        b.toggle(1, (0, 10));
        b.toggle(2, (1, 10));
        assert_eq!(b.get(1), Some((0, 10)));
        assert_eq!(b.get(2), Some((1, 10)));
    }

    #[test]
    fn clear_removes_every_bookmark() {
        let mut b = Marks::default();
        for n in [1, 3, 5] {
            b.toggle(n, (LEFT, n * 10));
        }
        b.clear();
        assert!(b.is_empty());
        assert_eq!(b.get(1), None);
    }

    #[test]
    fn map_and_retain_rewrite_anchors() {
        let mut b = Marks::default();
        b.toggle(1, (0, 5));
        b.toggle(2, (1, 7));
        b.map(|(s, l)| (1 - s, l));
        assert_eq!(b.get(1), Some((1, 5)));
        assert_eq!(b.get(2), Some((0, 7)));
        b.retain(|(s, _)| s != 1);
        assert_eq!(b.get(1), None);
        assert_eq!(b.get(2), Some((0, 7)));
    }

    fn numbered(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("line {i}")).collect()
    }
    fn refs(v: &[String]) -> Vec<&str> {
        v.iter().map(String::as_str).collect()
    }

    #[test]
    fn same_line_count_never_shifts() {
        let old = numbered(100);
        let mut new = old.clone();
        new[10] = "changed".into();
        new[60] = "changed too".into();
        assert_eq!(shift(&refs(&old)[..], &refs(&new)[..], 49), 49);
    }

    #[test]
    fn inserting_three_lines_above_moves_the_bookmark_down() {
        // 1-based 第 50 行 = 下标 49；在第 10 行处插入 3 行。
        let old = numbered(100);
        let mut new = old.clone();
        for i in 0..3 {
            new.insert(9, format!("inserted {i}"));
        }
        let at = shift(&refs(&old)[..], &refs(&new)[..], 49);
        assert_eq!(at, 52);
        assert_eq!(new[at], old[49], "still the same text");
    }

    #[test]
    fn deleting_the_bookmarked_line_lands_on_the_deletion_point() {
        // 删除 1-based 第 48–52 行（下标 47..52），书签在第 50 行 → 第 48 行。
        let old = numbered(100);
        let mut new = old.clone();
        new.drain(47..52);
        assert_eq!(shift(&refs(&old)[..], &refs(&new)[..], 49), 47);
    }

    #[test]
    fn deleting_to_the_end_of_file_clamps_to_the_last_line() {
        let old = numbered(100);
        let new = old[..90].to_vec();
        assert_eq!(shift(&refs(&old)[..], &refs(&new)[..], 95), 89);
        // 全部删光：落在第 0 行。
        assert_eq!(shift(&refs(&old)[..], &[][..], 95), 0);
    }

    #[test]
    fn bookmarks_after_the_change_shift_by_the_delta() {
        let old = numbered(100);
        let mut new = old.clone();
        new.drain(20..25);
        new.insert(20, "replacement".into());
        // 5 行换成 1 行：之后的行上移 4 行，之前的不动。
        assert_eq!(shift(&refs(&old)[..], &refs(&new)[..], 80), 76);
        assert_eq!(shift(&refs(&old)[..], &refs(&new)[..], 10), 10);
    }

    #[test]
    fn bookmarks_inside_the_change_keep_their_offset() {
        let old = numbered(100);
        let mut new = old.clone();
        // 下标 40..50 的 10 行换成 6 行。
        new.splice(40..50, (0..6).map(|i| format!("new {i}")));
        let (o, n) = (refs(&old), refs(&new));
        assert_eq!(shift(&o[..], &n[..], 42), 42, "offset 2 kept");
        assert_eq!(shift(&o[..], &n[..], 48), 45, "clamped to the new range's last line");
    }

    #[test]
    fn undo_shifts_back_by_the_same_rule() {
        let old = numbered(100);
        let mut new = old.clone();
        for i in 0..3 {
            new.insert(9, format!("inserted {i}"));
        }
        let (o, n) = (refs(&old), refs(&new));
        let moved = shift(&o[..], &n[..], 49);
        assert_eq!(shift(&n[..], &o[..], moved), 49);
    }
}
