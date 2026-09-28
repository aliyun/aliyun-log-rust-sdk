/// Uppercase hexadecimal context prefix and fixed-width hexadecimal sequence.
#[cfg(test)]
pub(crate) const PACK_ID_LEN: usize = 16 + 1 + 16;
pub(crate) const PACK_ID_TAG: &str = "__pack_id__";

/// Owned by the aggregator, one per active writer destination; no locks or global map.
pub(crate) struct PackIdGenerator {
    prefix: u64,
    sequence: u64,
}

impl PackIdGenerator {
    pub fn new() -> Self {
        Self {
            prefix: fastrand::u64(..),
            sequence: 0,
        }
    }

    pub fn next(&mut self) -> String {
        let id = format!("{:016X}-{:016X}", self.prefix, self.sequence);
        if let Some(next) = self.sequence.checked_add(1) {
            self.sequence = next;
        } else {
            *self = Self::new();
        }
        id
    }
}
