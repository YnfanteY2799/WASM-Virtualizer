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

// Fenwick Tree optimized for speed
struct FenwickTree {
    tree: Vec<f64>,
}

// Struct to represent a visible item, exposed to JavaScript
#[wasm_bindgen]
pub struct VisibleItem {
    pub index: u32,
    pub position: f64,
}

#[wasm_bindgen]
impl VirtualList {
    /// Creates a new VirtualList instance.
    ///
    /// # Arguments
    ///
    /// * `total_items` - Total number of items in the list (max: u32::MAX).
    /// * `estimated_size` - Initial estimated size for each item (must be non-negative).
    /// * `orientation` - Orientation of the list (Vertical or Horizontal).
    /// * `chunk_size` - Number of items per chunk (must be positive).
    ///
    /// # Panics
    ///
    /// Panics if:
    /// - `estimated_size` is negative.
    /// - `chunk_size` is zero.
    /// - Memory allocation would overflow due to `total_items` and `chunk_size`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let list = VirtualList::new(100, 50.0, Orientation::Vertical, 10);
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(
        total_items: u32,
        estimated_size: f64,
        orientation: Orientation,
        chunk_size: u32,
    ) -> Self {
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

        if total_items > usize::MAX / chunk_size {
            panic!(
                "Total items and chunk size combination would overflow memory allocation limits"
            );
        }

        let mut chunks = Vec::with_capacity((total_items + chunk_size - 1) / chunk_size);
        let mut cumulative_heights = Vec::with_capacity(chunks.capacity());
        let mut cumulative_height = 0.0;

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

    /// Updates the sizes of specified items.
    ///
    /// # Arguments
    ///
    /// * `indices` - Indices of items to update.
    /// * `sizes` - New sizes for the items (must match `indices` length).
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success.
    /// - `Err(String)` if:
    ///   - `indices` and `sizes` lengths differ.
    ///   - Any size is negative.
    ///   - Any index is out of bounds.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let mut list = VirtualList::new(5, 10.0, Orientation::Vertical, 2);
    /// list.update_item_sizes(&[0, 1], &[20.0, 30.0]).unwrap();
    /// ```
    pub fn update_item_sizes(&mut self, indices: &[u32], sizes: &[f64]) -> Result<(), String> {
        if indices.len() != sizes.len() {
            return Err(format!(
                "Indices length ({}) must match sizes length ({})",
                indices.len(),
                sizes.len()
            ));
        }
        for (&index, &size) in indices.iter().zip(sizes.iter()) {
            if size < 0.0 {
                return Err(format!(
                    "Item size must be non-negative, got {} at index {}",
                    size, index
                ));
            }
            let i = index as usize;
            if i >= self.total_items {
                return Err(format!(
                    "Index {} out of bounds, total items: {}",
                    i, self.total_items
                ));
            }
        }

        for (&index, &size) in indices.iter().zip(sizes.iter()) {
            let i = index as usize;
            let chunk_index = i / self.chunk_size;
            let chunk = unsafe { self.chunks.get_unchecked_mut(chunk_index) };
            let local_i = i - chunk.start;
            let delta = size - chunk.sizes[local_i];
            chunk.sizes[local_i] = size;
            chunk.tree.update(local_i, delta);
        }

        let mut cumulative_height = 0.0;
        for (i, chunk) in self.chunks.iter().enumerate() {
            unsafe {
                *self.cumulative_heights.get_unchecked_mut(i) = cumulative_height;
            }
            cumulative_height += chunk.tree.prefix_sum(chunk.sizes.len());
        }
        Ok(())
    }

    /// Computes the range of visible items based on scroll position and viewport size.
    ///
    /// # Arguments
    ///
    /// * `scroll_position` - Current scroll position (negative values treated as 0).
    /// * `viewport_size` - Size of the viewport (negative values treated as 0).
    /// * `overscan` - Number of extra items to include before and after the visible range.
    ///
    /// # Returns
    ///
    /// A vector of `VisibleItem` structs with the index and position of each visible item.
    /// Returns an empty vector if `total_items == 0`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let list = VirtualList::new(10, 50.0, Orientation::Vertical, 5);
    /// let visible = list.compute_visible_range(100.0, 200.0, 1);
    /// ```
    pub fn compute_visible_range(
        &self,
        scroll_position: f64,
        viewport_size: f64,
        overscan: u32,
    ) -> Vec<VisibleItem> {
        if self.total_items == 0 {
            return vec![];
        }
        let start = self.find_smallest_i_where_prefix_sum_ge(scroll_position);
        let end = self
            .find_largest_j_where_prefix_sum_le(scroll_position + viewport_size)
            .unwrap_or(0);
        let overscan = overscan as usize;
        let overscan_start = start.saturating_sub(overscan);
        let overscan_end = (end + overscan).min(self.total_items.saturating_sub(1));
        let mut visible_items = Vec::with_capacity(overscan_end - overscan_start + 1);
        for i in overscan_start..=overscan_end {
            let position = self.get_position(i);
            visible_items.push(VisibleItem {
                index: i as u32,
                position,
            });
        }
        visible_items
    }

    /// Gets the position of an item by its index.
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

    /// Finds the smallest index where the prefix sum is >= target.
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

    /// Finds the largest index where the prefix sum is <= target.
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

    /// Returns the estimated size of items.
    pub fn get_estimated_size(&self) -> f64 {
        self.estimated_size
    }

    /// Returns the orientation of the list.
    pub fn get_orientation(&self) -> Orientation {
        self.orientation
    }
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

