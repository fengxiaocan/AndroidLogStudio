use std::collections::VecDeque;

#[derive(Debug)]
pub struct RingBuffer<T> {
    capacity: usize,
    values: VecDeque<T>,
}

impl<T: Clone> RingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "ring buffer capacity must be greater than zero");
        Self { capacity, values: VecDeque::with_capacity(capacity) }
    }

    pub fn push(&mut self, value: T) {
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    pub fn latest(&self, limit: usize) -> Vec<T> {
        let start = self.values.len().saturating_sub(limit);
        self.values.iter().skip(start).cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_latest_values_when_full() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        buffer.push(4);

        assert_eq!(buffer.latest(10), vec![2, 3, 4]);
    }

    #[test]
    fn latest_respects_limit() {
        let mut buffer = RingBuffer::new(5);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        assert_eq!(buffer.latest(2), vec![2, 3]);
    }
}
