/// 空でないバイト範囲。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NonemptyRange {
    min: usize,
    max: usize,
}

impl NonemptyRange {
    pub fn from_min_max(min: usize, max: usize) -> Self {
        assert!(min <= max);

        Self { min, max }
    }

    pub fn from_start_len(start: usize, len: usize) -> Self {
        assert!(len > 0);

        Self::from_min_max(start, start + len - 1)
    }

    pub fn min(self) -> usize {
        self.min
    }

    pub fn max(self) -> usize {
        self.max
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(self) -> usize {
        self.max - self.min + 1
    }

    pub fn intersects(self, other: Self) -> bool {
        !(self.max < other.min || other.max < self.min)
    }

    pub fn contains(self, x: usize) -> bool {
        self.min <= x && x <= self.max
    }

    pub fn contains_range(self, other: Self) -> bool {
        self.min <= other.min && other.max <= self.max
    }
}
