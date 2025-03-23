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
    /// Creates a new VirtualList instance
    #[wasm_bindgen(constructor)]
    pub fn new(
        total_items: u32,
        estimated_size: f64,
        orientation: Orientation,
        chunk_size: u32,
    ) -> Self {
        let total_items = total_items as usize;
        let chunk_size = chunk_size as usize;
        let mut chunks = Vec::new();
        let mut cumulative_height = 0.0;
        let mut cumulative_heights = Vec::new();

        // Create chunks with ranges and initialize sizes
        for start in (0..total_items).step_by(chunk_size) {
            let end = (start + chunk_size).min(total_items);
            // Line specified by user: initialize Fenwick Tree with chunk size
            let mut tree = FenwickTree::new(end - start);
            let sizes = vec![estimated_size; end - start];
            // Populate the Fenwick Tree with initial sizes
            for i in 0..(end - start) {
                tree.update(i, estimated_size);
            }
            chunks.push(Chunk {
                start,
                end,
                tree,
                sizes,
            });
            cumulative_heights.push(cumulative_height);
            cumulative_height += (end - start) as f64 * estimated_size;
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

    /// Updates the sizes of specified items and recalculates cumulative heights
    pub fn update_item_sizes(&mut self, indices: &[u32], sizes: &[f64]) {
        for (&index, &size) in indices.iter().zip(sizes.iter()) {
            let i = index as usize;
            if i < self.total_items {
                let chunk_index = i / self.chunk_size;
                if let Some(chunk) = self.chunks.get_mut(chunk_index) {
                    // Use chunk.start to compute local index
                    let local_i = i - chunk.start;
                    let delta = size - chunk.sizes[local_i];
                    chunk.tree.update(local_i, delta);
                    chunk.sizes[local_i] = size;
                }
            }
        }
        // Update cumulative heights after size changes
        let mut cumulative_height = 0.0;
        for (i, chunk) in self.chunks.iter().enumerate() {
            self.cumulative_heights[i] = cumulative_height;
            // Total height of the chunk
            cumulative_height += chunk.tree.prefix_sum(chunk.sizes.len());
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
        let end = self
            .find_largest_j_where_prefix_sum_le(scroll_position + viewport_size)
            .unwrap_or(0);
        let overscan_start = start.saturating_sub(overscan as usize);
        let overscan_end = (end + overscan as usize).min(self.total_items.saturating_sub(1));
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

    /// Gets the global position of an item
    fn get_position(&self, index: usize) -> f64 {
        if index >= self.total_items {
            return 0.0;
        }
        let chunk_index = index / self.chunk_size;
        if let Some(chunk) = self.chunks.get(chunk_index) {
            // Use chunk.start to compute local index
            let local_i = index - chunk.start;
            // Global position = cumulative height before this chunk + position within chunk
            self.cumulative_heights[chunk_index] + chunk.tree.prefix_sum(local_i)
        } else {
            0.0
        }
    }

    /// Finds the smallest index where the prefix sum >= target
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

    /// Finds the largest index where the prefix sum <= target
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

    /// Getter for estimated_size to ensure it’s used
    pub fn get_estimated_size(&self) -> f64 {
        self.estimated_size
    }

    /// Getter for orientation to ensure it’s used
    pub fn get_orientation(&self) -> Orientation {
        self.orientation
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
        index += 1; // Fenwick Tree uses 1-based indexing internally
        while index < self.tree.len() {
            self.tree[index] += delta;
            index += index & index.wrapping_neg(); // Move to next relevant index
        }
    }

    fn prefix_sum(&self, mut index: usize) -> f64 {
        let mut sum = 0.0;
        while index > 0 {
            sum += self.tree[index];
            index -= index & index.wrapping_neg(); // Move to parent
        }
        sum
    }
}
