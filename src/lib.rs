use std::cmp;
use wasm_bindgen::prelude::*;

// Define the Orientation enum for the list
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

// Define errors that can be returned
#[wasm_bindgen]
#[derive(Debug)]
pub enum VirtualListError {
    IndexOutOfBounds,
    InvalidSize,
    InvalidViewport,
}

// Helper function to convert errors to JS
#[wasm_bindgen]
pub fn get_error_message(error: VirtualListError) -> String {
    match error {
        VirtualListError::IndexOutOfBounds => "Index out of bounds".to_string(),
        VirtualListError::InvalidSize => "Invalid size provided".to_string(),
        VirtualListError::InvalidViewport => "Invalid viewport size".to_string(),
    }
}

// Define a struct to return visible range results that's compatible with wasm_bindgen
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct VisibleRange {
    start: usize,
    end: usize,
}

#[wasm_bindgen]
impl VisibleRange {
    #[wasm_bindgen(getter)]
    pub fn start(&self) -> usize {
        self.start
    }

    #[wasm_bindgen(getter)]
    pub fn end(&self) -> usize {
        self.end
    }
}

// Define the Chunk struct to hold item sizes
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct Chunk {
    sizes: Vec<f64>,       // Sizes of items in this chunk
    chunk_total_size: f64, // Cached total size of this chunk
}

#[wasm_bindgen]
impl Chunk {
    #[wasm_bindgen(constructor)]
    pub fn new(chunk_size: usize, estimated_size: f64) -> Chunk {
        let sizes = vec![estimated_size.max(0.0); chunk_size]; // Ensure sizes are non-negative
        let chunk_total_size = estimated_size.max(0.0) * chunk_size as f64;

        Chunk {
            sizes,
            chunk_total_size,
        }
    }

    // Internal method to get the total size without recomputing
    fn get_total_size(&self) -> f64 {
        self.chunk_total_size
    }

    // Update an item size and the chunk's total size
    fn update_size(&mut self, index: usize, new_size: f64) -> Result<f64, VirtualListError> {
        if index >= self.sizes.len() {
            return Err(VirtualListError::IndexOutOfBounds);
        }

        if new_size < 0.0 {
            return Err(VirtualListError::InvalidSize);
        }

        let size_diff = new_size - self.sizes[index];
        self.sizes[index] = new_size;
        self.chunk_total_size += size_diff;

        Ok(size_diff)
    }

    // Get the sum of sizes up to a specific index within the chunk
    fn get_position_in_chunk(&self, index: usize) -> Result<f64, VirtualListError> {
        if index > self.sizes.len() {
            return Err(VirtualListError::IndexOutOfBounds);
        }

        let position = self.sizes[..index].iter().sum();
        Ok(position)
    }
}

// Define the VirtualList struct
#[wasm_bindgen]
pub struct VirtualList {
    total_items: usize, // Total number of items in the list
    #[allow(dead_code)]
    estimated_size: f64, // Default size for unmeasured items
    orientation: Orientation, // List orientation
    chunks: Vec<Chunk>, // Chunks of items for efficient management
    chunk_size: usize,  // Number of items per chunk
    cumulative_sizes: Vec<f64>, // Cumulative sizes up to each chunk
    total_size: f64,    // Total size of all items (cached for performance)
}

#[wasm_bindgen]
impl VirtualList {
    /// Constructor for VirtualList
    #[wasm_bindgen(constructor)]
    pub fn new(
        total_items: usize,
        chunk_size: usize,
        estimated_size: f64,
        orientation: Orientation,
    ) -> VirtualList {
        // Validate inputs
        let chunk_size = cmp::max(1, chunk_size); // Ensure chunk size is at least 1
        let estimated_size = estimated_size.max(0.0); // Ensure estimated size is non-negative

        let num_chunks = (total_items + chunk_size - 1) / chunk_size; // Ceiling division
        let mut chunks = Vec::with_capacity(num_chunks);
        let mut cumulative_sizes = Vec::with_capacity(num_chunks);

        let mut running_total = 0.0;
        for i in 0..num_chunks {
            let items_in_chunk = if i == num_chunks - 1 {
                // Last chunk might have fewer items
                total_items - (i * chunk_size)
            } else {
                chunk_size
            };

            let chunk = Chunk::new(items_in_chunk, estimated_size);
            running_total += chunk.get_total_size();
            chunks.push(chunk);
            cumulative_sizes.push(running_total);
        }

        VirtualList {
            total_items,
            estimated_size,
            orientation,
            chunks,
            chunk_size,
            cumulative_sizes,
            total_size: running_total,
        }
    }

    /// Get the total size of the list
    pub fn get_total_size(&self) -> f64 {
        self.total_size
    }

    /// Get the number of items in the list
    pub fn get_total_items(&self) -> usize {
        self.total_items
    }

    /// Get the orientation of the list
    pub fn get_orientation(&self) -> Orientation {
        self.orientation
    }

    /// Set the orientation of the list
    pub fn set_orientation(&mut self, orientation: Orientation) {
        self.orientation = orientation;
    }

    /// Get the position of an item in the list
    #[wasm_bindgen]
    pub fn get_position(&self, index: usize) -> Result<f64, JsValue> {
        if index >= self.total_items {
            return Err(JsValue::from_str(&get_error_message(
                VirtualListError::IndexOutOfBounds,
            )));
        }

        let chunk_idx = index / self.chunk_size;
        let item_idx_in_chunk = index % self.chunk_size;

        // Position is the cumulative size up to the previous chunk plus the size of items in the current chunk up to the index
        let prev_size = if chunk_idx > 0 {
            self.cumulative_sizes[chunk_idx - 1]
        } else {
            0.0
        };

        let chunk = &self.chunks[chunk_idx];
        match chunk.get_position_in_chunk(item_idx_in_chunk) {
            Ok(position_in_chunk) => Ok(prev_size + position_in_chunk),
            Err(e) => Err(JsValue::from_str(&get_error_message(e))),
        }
    }

    /// Update the size of an item and recalculate cumulative sizes
    #[wasm_bindgen]
    pub fn update_item_size(&mut self, index: usize, new_size: f64) -> Result<(), JsValue> {
        if index >= self.total_items {
            return Err(JsValue::from_str(&get_error_message(
                VirtualListError::IndexOutOfBounds,
            )));
        }

        if new_size < 0.0 {
            return Err(JsValue::from_str(&get_error_message(
                VirtualListError::InvalidSize,
            )));
        }

        let chunk_idx = index / self.chunk_size;
        let item_idx_in_chunk = index % self.chunk_size;

        // Update the size and get the difference
        let chunk = &mut self.chunks[chunk_idx];
        let size_diff = match chunk.update_size(item_idx_in_chunk, new_size) {
            Ok(diff) => diff,
            Err(e) => return Err(JsValue::from_str(&get_error_message(e))),
        };

        // If size didn't change, no need to update cumulative sizes
        if size_diff == 0.0 {
            return Ok(());
        }

        // Update total size
        self.total_size += size_diff;

        // Only update cumulative sizes from this chunk onward
        for i in chunk_idx..self.chunks.len() {
            self.cumulative_sizes[i] += size_diff;
        }

        Ok(())
    }

    /// Estimate visible items based on viewport size and scroll position
    /// Returns a VisibleRange object with start and end indices
    #[wasm_bindgen]
    pub fn get_visible_range(
        &self,
        scroll_position: f64,
        viewport_size: f64,
    ) -> Result<VisibleRange, JsValue> {
        if viewport_size <= 0.0 {
            return Err(JsValue::from_str(&get_error_message(
                VirtualListError::InvalidViewport,
            )));
        }

        // Find the first visible item
        let start_idx = self.binary_search_position(scroll_position);

        // Find the last visible item
        let end_position = scroll_position + viewport_size;
        let end_idx = self.binary_search_position(end_position);

        // Add buffer items for smoother scrolling (can be adjusted)
        let buffer = 5;
        let start = start_idx.saturating_sub(buffer);
        let end = cmp::min(end_idx + buffer, self.total_items);

        Ok(VisibleRange { start, end })
    }

    /// Binary search to find the item at a given position
    fn binary_search_position(&self, position: f64) -> usize {
        // Handle edge cases
        if position <= 0.0 {
            return 0;
        }
        if position >= self.total_size {
            return self.total_items.saturating_sub(1);
        }

        // First find the chunk using binary search
        let mut low = 0;
        let mut high = self.chunks.len() - 1;

        while low <= high {
            let mid = (low + high) / 2;
            let mid_pos = self.cumulative_sizes[mid];

            if mid == 0 || (position > self.cumulative_sizes[mid - 1] && position <= mid_pos) {
                // Found the chunk, now find the item within the chunk
                let chunk = &self.chunks[mid];
                let chunk_start_pos = if mid > 0 {
                    self.cumulative_sizes[mid - 1]
                } else {
                    0.0
                };
                let position_in_chunk = position - chunk_start_pos;

                // Linear search within the chunk (could optimize with binary search if needed)
                let mut running_pos = 0.0;
                for i in 0..chunk.sizes.len() {
                    running_pos += chunk.sizes[i];
                    if running_pos >= position_in_chunk {
                        return mid * self.chunk_size + i;
                    }
                }

                // Fallback - return last item in chunk
                return mid * self.chunk_size + chunk.sizes.len() - 1;
            }

            if position <= mid_pos {
                if high == mid && high > 0 {
                    high -= 1; // Prevent infinite loop
                } else if high > mid {
                    high = mid;
                } else {
                    break;
                }
            } else {
                if low == mid && low < high {
                    low += 1; // Prevent infinite loop
                } else if low < mid {
                    low = mid;
                } else {
                    break;
                }
            }
        }

        // Fallback
        return (position as usize * self.total_items) / self.total_size as usize;
    }

    /// Get the size of an item at a specific index
    #[wasm_bindgen]
    pub fn get_item_size(&self, index: usize) -> Result<f64, JsValue> {
        if index >= self.total_items {
            return Err(JsValue::from_str(&get_error_message(
                VirtualListError::IndexOutOfBounds,
            )));
        }

        let chunk_idx = index / self.chunk_size;
        let item_idx_in_chunk = index % self.chunk_size;

        if chunk_idx < self.chunks.len() && item_idx_in_chunk < self.chunks[chunk_idx].sizes.len() {
            Ok(self.chunks[chunk_idx].sizes[item_idx_in_chunk])
        } else {
            Err(JsValue::from_str(&get_error_message(
                VirtualListError::IndexOutOfBounds,
            )))
        }
    }
}

// Add tests for the implementation
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_virtual_list() {
        let list = VirtualList::new(100, 10, 50.0, Orientation::Vertical);
        assert_eq!(list.get_total_items(), 100);
        assert_eq!(list.get_total_size(), 5000.0);
    }

    #[test]
    fn test_update_item_size() {
        let mut list = VirtualList::new(100, 10, 50.0, Orientation::Vertical);
        let initial_size = list.get_total_size();

        // Update an item size
        list.update_item_size(5, 100.0).unwrap();

        // Total size should have increased by 50.0 (100.0 - 50.0)
        assert_eq!(list.get_total_size(), initial_size + 50.0);

        // Position of item 5 should be the sum of sizes 0-4
        let pos = list.get_position(5).unwrap();
        assert_eq!(pos, 250.0); // 5 * 50.0

        // Position of item 6 should include the new size of item 5
        let pos = list.get_position(6).unwrap();
        assert_eq!(pos, 350.0); // (5 * 50.0) + 100.0
    }

    #[test]
    fn test_get_visible_range() {
        let list = VirtualList::new(100, 10, 50.0, Orientation::Vertical);

        // Viewport at the beginning
        let visible_range = list.get_visible_range(0.0, 200.0).unwrap();
        assert_eq!(visible_range.start, 0);
        assert!(visible_range.end >= 4); // At least items 0-3 should be visible + buffer

        // Viewport in the middle
        let visible_range = list.get_visible_range(2000.0, 200.0).unwrap();
        assert!(visible_range.start >= 35);
        assert!(visible_range.end <= 50);
    }

    #[test]
    fn test_get_item_size() {
        let mut list = VirtualList::new(100, 10, 50.0, Orientation::Vertical);

        // Initial size
        let size = list.get_item_size(5).unwrap();
        assert_eq!(size, 50.0);

        // Update and check
        list.update_item_size(5, 75.0).unwrap();
        let size = list.get_item_size(5).unwrap();
        assert_eq!(size, 75.0);
    }
}
