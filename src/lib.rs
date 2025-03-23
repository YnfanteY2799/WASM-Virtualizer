use wasm_bindgen::prelude::*;

// Orientation enum for horizontal or vertical lists
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

// Custom error type for precise error handling
#[derive(Debug)]
pub enum VirtualListError {
    InvalidInput(String),
    IndexOutOfBounds(usize),
}

// Structure to hold a chunk of items with their sizes and prefix sums
#[derive(Clone)]
struct Chunk {
    start: usize,           // Starting index of the chunk
    tree: FenwickTree,      // Fenwick Tree for prefix sum calculations
    sizes: Vec<f64>,        // Sizes of items in the chunk
}

// Fenwick Tree for efficient prefix sum updates and queries
#[derive(Clone)]
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
            index += index & (!index + 1); // Move to next relevant index
        }
    }

    fn prefix_sum(&self, mut index: usize) -> f64 {
        let mut sum = 0.0;
        while index > 0 {
            sum += self.tree[index];
            index -= index & (!index + 1); // Move to parent index
        }
        sum
    }
}

// Visible item structure for WebAssembly binding
#[wasm_bindgen]
pub struct VisibleItem {
    index: u32,
    position: f64,
}

#[wasm_bindgen]
impl VisibleItem {
    #[wasm_bindgen(getter)]
    pub fn index(&self) -> u32 {
        self.index
    }

    #[wasm_bindgen(getter)]
    pub fn position(&self) -> f64 {
        self.position
    }
}

// Main VirtualList struct
#[wasm_bindgen]
pub struct VirtualList {
    total_items: usize,         // Total number of items in the list
    estimated_size: f64,        // Default size for unmeasured items
    orientation: Orientation,   // List orientation (vertical or horizontal)
    chunks: Vec<Chunk>,         // Chunks of items for efficient management
    chunk_size: usize,          // Number of items per chunk
    cumulative_sizes: Vec<f64>, // Cumulative sizes up to each chunk
}

#[wasm_bindgen]
impl VirtualList {
    /// Creates a new VirtualList with the specified parameters.
    /// Panics if `estimated_size` is negative or `chunk_size` is zero.
    #[wasm_bindgen(constructor)]
    pub fn new(
        total_items: u32,
        estimated_size: f64,
        orientation: Orientation,
        chunk_size: u32,
    ) -> Self {
        if estimated_size < 0.0 {
            panic!("Estimated size must be non-negative, got {}", estimated_size);
        }
        if chunk_size == 0 {
            panic!("Chunk size must be positive, got {}", chunk_size);
        }
        let total_items = total_items as usize;
        let chunk_size = chunk_size as usize;

        if total_items > usize::MAX / chunk_size {
            panic!("Total items and chunk size combination would overflow memory allocation limits");
        }

        let mut chunks = Vec::with_capacity((total_items + chunk_size - 1) / chunk_size);
        let mut cumulative_sizes = Vec::with_capacity(chunks.capacity());
        let mut cumulative_size = 0.0;

        for start in (0..total_items).step_by(chunk_size) {
            let end = (start + chunk_size).min(total_items);
            let chunk_len = end - start;
            let mut tree = FenwickTree::new(chunk_len);
            let sizes = vec![estimated_size; chunk_len];
            for i in 0..chunk_len {
                tree.tree[i + 1] = estimated_size * (i + 1) as f64;
            }
            chunks.push(Chunk {
                start,
                tree,
                sizes,
            });
            cumulative_sizes.push(cumulative_size);
            cumulative_size += estimated_size * chunk_len as f64;
        }

        VirtualList {
            total_items,
            estimated_size,
            orientation,
            chunks,
            chunk_size,
            cumulative_sizes,
        }
    }

    /// Updates the sizes of items at the specified indices.
    /// Returns an error if indices and sizes lengths don't match, sizes are negative,
    /// or indices are out of bounds.
    pub fn update_item_sizes(&mut self, indices: &[u32], sizes: &[f64]) -> Result<(), VirtualListError> {
        if indices.len() != sizes.len() {
            return Err(VirtualListError::InvalidInput(format!(
                "Indices length ({}) must match sizes length ({})",
                indices.len(),
                sizes.len()
            )));
        }
        for (&index, &size) in indices.iter().zip(sizes.iter()) {
            if size < 0.0 {
                return Err(VirtualListError::InvalidInput(format!(
                    "Item size must be non-negative, got {} at index {}",
                    size, index
                )));
            }
            let i = index as usize;
            if i >= self.total_items {
                return Err(VirtualListError::IndexOutOfBounds(i));
            }
        }

        for (&index, &size) in indices.iter().zip(sizes.iter()) {
            let i = index as usize;
            let chunk_index = i / self.chunk_size;
            let chunk = &mut self.chunks[chunk_index]; // Safe indexing
            let local_i = i - chunk.start;
            let delta = size - chunk.sizes[local_i];
            chunk.sizes[local_i] = size;
            chunk.tree.update(local_i, delta);
        }

        let mut cumulative_size = 0.0;
        for (i, chunk) in self.chunks.iter().enumerate() {
            self.cumulative_sizes[i] = cumulative_size;
            cumulative_size += chunk.tree.prefix_sum(chunk.sizes.len());
        }
        Ok(())
    }

    /// Computes the range of visible items based on the scroll position and viewport size.
    /// Returns a vector of `VisibleItem` containing the index and position of each visible item.
    pub fn compute_visible_range(
        &self,
        scroll_position: f64,
        viewport_size: f64,
        overscan: u32,
    ) -> Vec<VisibleItem> {
        if self.total_items == 0 || viewport_size <= 0.0 {
            return Vec::new();
        }

        let start_pos = scroll_position.max(0.0);
        let end_pos = start_pos + viewport_size;

        let start = self.find_smallest_i_where_prefix_sum_ge(start_pos);
        let end = self.find_largest_j_where_prefix_sum_le(end_pos);

        if start >= self.total_items {
            return Vec::new();
        }

        let overscan_start = start.saturating_sub(overscan as usize);
        let overscan_end = (end + overscan as usize).min(self.total_items.saturating_sub(1));

        let mut visible = Vec::with_capacity(overscan_end - overscan_start + 1);
        for i in overscan_start..=overscan_end {
            let position = self.get_position(i);
            visible.push(VisibleItem {
                index: i as u32,
                position,
            });
        }
        visible
    }

    /// Gets the position of an item by its index.
    /// Assumes that index < total_items; debug assertion enforces this in debug builds.
    fn get_position(&self, index: usize) -> f64 {
        debug_assert!(index < self.total_items, "Index {} out of bounds, total_items: {}", index, self.total_items);
        let chunk_index = index / self.chunk_size;
        let chunk = &self.chunks[chunk_index];
        let local_i = index - chunk.start;
        self.cumulative_sizes[chunk_index] + chunk.tree.prefix_sum(local_i)
    }

    /// Finds the smallest index where the prefix sum >= target.
    /// Assumes that sizes are non-negative, so prefix sums are non-decreasing.
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

    /// Finds the largest index where the prefix sum <= target.
    /// Assumes that sizes are non-negative, so prefix sums are non-decreasing.
    fn find_largest_j_where_prefix_sum_le(&self, target: f64) -> usize {
        if target < 0.0 || self.total_items == 0 {
            return 0;
        }
        let mut low = 0;
        let mut high = self.total_items;
        while low < high {
            let mid = low + (high - low) / 2;
            let pos = self.get_position(mid);
            if pos <= target {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        high.saturating_sub(1)
    }
}