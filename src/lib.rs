use wasm_bindgen::prelude::*;

// Orientation enum for vertical or horizontal scrolling
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

// Represents a chunk of items for memory efficiency
struct Chunk {
    start: usize,      // Starting global index of the chunk
    end: usize,        // Exclusive ending global index of the chunk
    tree: FenwickTree, // Fenwick Tree for prefix sums within the chunk
    sizes: Vec<f64>,   // Sizes of items in this chunk
}

// Main struct for virtualization logic
#[wasm_bindgen]
pub struct VirtualList {
    total_items: usize,           // Total number of items in the list
    estimated_size: f64,          // Default size for items
    orientation: Orientation,     // List orientation (vertical or horizontal)
    chunks: Vec<Chunk>,           // List of chunks
    chunk_size: usize,            // Number of items per chunk (except possibly the last)
    cumulative_heights: Vec<f64>, // Cumulative height up to each chunk
}

#[wasm_bindgen]
impl VirtualList {
    /// Creates a new VirtualList instance with robust input validation
    #[wasm_bindgen(constructor)]
    pub fn new(
        total_items: u32,
        estimated_size: f64,
        orientation: Orientation,
        chunk_size: u32,
    ) -> Self {
        // Validate inputs
        if estimated_size < 0.0 {
            panic!(
                "Estimated size must be non-negative, got {}",
                estimated_size
            );
        }
        if chunk_size == 0 {
            panic!("Chunk size must be positive, got {}", chunk_size);
        }
        let total_items = total_items as usize;
        let chunk_size = chunk_size as usize;

        // Prevent excessive memory allocation
        if total_items > usize::MAX / chunk_size {
            panic!(
                "Total items and chunk size combination would overflow memory allocation limits"
            );
        }

        let mut chunks = Vec::with_capacity((total_items + chunk_size - 1) / chunk_size);
        let mut cumulative_heights = Vec::with_capacity(chunks.capacity());
        let mut cumulative_height = 0.0;

        // Create chunks efficiently
        for start in (0..total_items).step_by(chunk_size) {
            let end = (start + chunk_size).min(total_items);
            let chunk_len = end - start;
            let mut tree = FenwickTree::new(chunk_len);
            let sizes = vec![estimated_size; chunk_len];
            // Inline initialization for speed
            for i in 0..chunk_len {
                tree.tree[i + 1] = estimated_size * (i + 1) as f64;
            }
            chunks.push(Chunk {
                start,
                end,
                tree,
                sizes,
            });
            cumulative_heights.push(cumulative_height);
            cumulative_height += estimated_size * chunk_len as f64;
        }

        VirtualList {
            total_items,
            estimated_size,
            orientation,
            chunks,
            chunk_size,
            cumulative_heights,
        }
    }

    /// Updates item sizes with comprehensive error checking and optimized updates
    pub fn update_item_sizes(&mut self, indices: &[u32], sizes: &[f64]) {
        // Validate input arrays
        if indices.len() != sizes.len() {
            panic!(
                "Indices length ({}) must match sizes length ({})",
                indices.len(),
                sizes.len()
            );
        }
        if indices.is_empty() {
            return; // Early return for empty input
        }

        // Validate each size and index
        for (&index, &size) in indices.iter().zip(sizes.iter()) {
            if size < 0.0 {
                panic!(
                    "Item size must be non-negative, got {} at index {}",
                    size, index
                );
            }
            let i = index as usize;
            if i >= self.total_items {
                panic!(
                    "Index {} out of bounds, total items: {}",
                    i, self.total_items
                );
            }
        }

        // Batch updates for efficiency
        for (&index, &size) in indices.iter().zip(sizes.iter()) {
            let i = index as usize;
            let chunk_index = i / self.chunk_size;
            let chunk = unsafe { self.chunks.get_unchecked_mut(chunk_index) }; // Bounds checked above
            let local_i = i - chunk.start;
            let delta = size - chunk.sizes[local_i];
            chunk.sizes[local_i] = size;
            chunk.tree.update(local_i, delta);
        }

        // Optimize cumulative heights update
        let mut cumulative_height = 0.0;
        for (i, chunk) in self.chunks.iter().enumerate() {
            unsafe {
                *self.cumulative_heights.get_unchecked_mut(i) = cumulative_height;
            }
            cumulative_height += chunk.tree.prefix_sum(chunk.sizes.len());
        }
    }

    /// Computes visible range with robust checks and high performance
    pub fn compute_visible_range(
        &self,
        scroll_position: f64,
        viewport_size: f64,
        overscan: u32,
        output: &mut [f64],
    ) -> u32 {
        // Validate inputs
        if scroll_position < 0.0 {
            panic!(
                "Scroll position must be non-negative, got {}",
                scroll_position
            );
        }
        if viewport_size < 0.0 {
            panic!("Viewport size must be non-negative, got {}", viewport_size);
        }

        let start = self.find_smallest_i_where_prefix_sum_ge(scroll_position);
        let end = self
            .find_largest_j_where_prefix_sum_le(scroll_position + viewport_size)
            .unwrap_or(0);
        let overscan = overscan as usize;
        let overscan_start = start.saturating_sub(overscan);
        let overscan_end = (end + overscan).min(self.total_items.saturating_sub(1));
        let item_count = overscan_end - overscan_start + 1;
        let required_buffer_size = item_count * 2;

        if output.len() < required_buffer_size {
            panic!(
                "Output buffer too small: need {} elements, got {}",
                required_buffer_size,
                output.len()
            );
        }

        // Fast population of output buffer
        let mut count = 0;
        for i in overscan_start..=overscan_end {
            unsafe {
                *output.get_unchecked_mut(count * 2) = i as f64; // Index
                *output.get_unchecked_mut(count * 2 + 1) = self.get_position(i);
                // Position
            }
            count += 1;
        }
        count as u32
    }

    /// Gets item position with safety checks
    fn get_position(&self, index: usize) -> f64 {
        if index >= self.total_items {
            panic!(
                "Index {} out of bounds, total items: {}",
                index, self.total_items
            );
        }
        let chunk_index = index / self.chunk_size;
        let chunk = unsafe { self.chunks.get_unchecked(chunk_index) };
        let local_i = index - chunk.start;
        unsafe {
            *self.cumulative_heights.get_unchecked(chunk_index) + chunk.tree.prefix_sum(local_i)
        }
    }

    /// Optimized binary search for smallest index where prefix sum >= target
    fn find_smallest_i_where_prefix_sum_ge(&self, target: f64) -> usize {
        if target <= 0.0 {
            return 0;
        }
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

    /// Optimized binary search for largest index where prefix sum <= target
    fn find_largest_j_where_prefix_sum_le(&self, target: f64) -> Option<usize> {
        if target < 0.0 {
            return None;
        }
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

    /// Getter for estimated_size
    pub fn get_estimated_size(&self) -> f64 {
        self.estimated_size
    }

    /// Getter for orientation
    pub fn get_orientation(&self) -> Orientation {
        self.orientation
    }
}

// Fenwick Tree optimized for speed
struct FenwickTree {
    tree: Vec<f64>,
}

impl FenwickTree {
    fn new(size: usize) -> Self {
        FenwickTree {
            tree: vec![0.0; size + 1],
        }
    }

    #[inline]
    fn update(&mut self, mut index: usize, delta: f64) {
        index += 1;
        while index < self.tree.len() {
            unsafe {
                *self.tree.get_unchecked_mut(index) += delta;
            }
            index += index & index.wrapping_neg();
        }
    }

    #[inline]
    fn prefix_sum(&self, mut index: usize) -> f64 {
        let mut sum = 0.0;
        while index > 0 {
            unsafe {
                sum += *self.tree.get_unchecked(index);
            }
            index -= index & index.wrapping_neg();
        }
        sum
    }
}
