use wasm_bindgen::prelude::*;
use std::collections::HashMap;

// Orientation enum for vertical or horizontal scrolling
#[wasm_bindgen]
pub enum Orientation {
    Vertical,
    Horizontal,
}

// Represents a chunk of items for memory efficiency
struct Chunk {
    start: usize,
    end: usize,
    tree: FenwickTree,
    sizes: Vec<f64>,
}

// Main struct for virtualization logic
#[wasm_bindgen]
pub struct VirtualList {
    total_items: usize,
    estimated_size: f64,
    orientation: Orientation,
    chunks: Vec<Chunk>,
    chunk_size: usize,
    sparse_sizes: HashMap<usize, f64>, // For items with non-default sizes
}

#[wasm_bindgen]
impl VirtualList {
    /// Creates a new VirtualList instance
    #[wasm_bindgen(constructor)]
    pub fn new(total_items: u32, estimated_size: f64, orientation: Orientation, chunk_size: u32) -> Self {
        let total_items = total_items as usize;
        let chunk_size = chunk_size as usize;
        let mut chunks = Vec::new();
        for start in (0..total_items).step_by(chunk_size) {
            let end = (start + chunk_size).min(total_items);
            let mut tree = FenwickTree::new(end - start);
            let sizes = vec![estimated_size; end - start];
            for i in start..end {
                tree.update(i - start, estimated_size);
            }
            chunks.push(Chunk { start, end, tree, sizes });
        }
        VirtualList {
            total_items,
            estimated_size,
            orientation,
            chunks,
            chunk_size,
            sparse_sizes: HashMap::new(),
        }
    }

    /// Updates the sizes of specified items
    pub fn update_item_sizes(&mut self, indices: &[u32], sizes: &[f64]) {
        for (&index, &size) in indices.iter().zip(sizes.iter()) {
            let i = index as usize;
            if i < self.total_items {
                self.sparse_sizes.insert(i, size);
                // Update the corresponding chunk
                let chunk_index = i / self.chunk_size;
                if let Some(chunk) = self.chunks.get_mut(chunk_index) {
                    let local_i = i % self.chunk_size;
                    let delta = size - chunk.sizes[local_i];
                    chunk.tree.update(local_i, delta);
                    chunk.sizes[local_i] = size;
                }
            }
        }
    }

    /// Computes the visible range of items and writes (index, position) pairs to the output buffer
    pub fn compute_visible_range(
        &self,
        scroll_position: f64,
        viewport_size: f64,
        overscan: u32,
        output: &mut [f64],
    ) -> u32 {
        let start = self.find_smallest_i_where_prefix_sum_ge(scroll_position);
        let end = self.find_largest_j_where_prefix_sum_le(scroll_position + viewport_size).unwrap_or(0);
        let overscan_start = start.saturating_sub(overscan as usize);
        let overscan_end = (end + overscan as usize).min(self.total_items - 1);
        let mut count = 0;
        for i in overscan_start..=overscan_end {
            if count * 2 + 1 < output.len() {
                output[count * 2] = i as f64; // Index
                output[count * 2 + 1] = self.get_position(i); // Position
                count += 1;
            } else {
                break;
            }
        }
        count as u32
    }

    // Helper to get the position of an item
    fn get_position(&self, index: usize) -> f64 {
        let chunk_index = index / self.chunk_size;
        if let Some(chunk) = self.chunks.get(chunk_index) {
            let local_i = index % self.chunk_size;
            chunk.tree.prefix_sum(local_i)
        } else {
            0.0
        }
    }

    // Binary search to find the smallest index where prefix sum >= target
    fn find_smallest_i_where_prefix_sum_ge(&self, target: f64) -> usize {
        let mut low = 0;
        let mut high = self.total_items;
        while low < high {
            let mid = low + (high - low) / 2;
            let pos = self.get_position(mid);
            if pos < target {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        low
    }

    // Binary search to find the largest index where prefix sum <= target
    fn find_largest_j_where_prefix_sum_le(&self, target: f64) -> Option<usize> {
        let mut low = 0;
        let mut high = self.total_items;
        let mut result = None;
        while low < high {
            let mid = low + (high - low) / 2;
            let pos = self.get_position(mid);
            if pos <= target {
                result = Some(mid);
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        result
    }
}

// Fenwick Tree for efficient prefix sum calculations
struct FenwickTree {
    tree: Vec<f64>,
}

impl FenwickTree {
    fn new(size: usize) -> Self {
        FenwickTree {
            tree: vec![0.0; size + 1],
        }
    }

    fn update(&mut self, mut index: usize, delta: f64) {
        index += 1;
        while index < self.tree.len() {
            self.tree[index] += delta;
            index += index & index.wrapping_neg();
        }
    }

    fn prefix_sum(&self, mut index: usize) -> f64 {
        let mut sum = 0.0;
        while index > 0 {
            sum += self.tree[index];
            index -= index & index.wrapping_neg();
        }
        sum
    }
}