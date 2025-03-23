use std::cmp;
use wasm_bindgen::prelude::*;

// Define the Orientation enum for the list
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

// Define errors that can be returned
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub enum VirtualListError {
    IndexOutOfBounds,
    InvalidSize,
    InvalidViewport,
    InvalidConfiguration,
    EmptyList,
}

// Helper function to convert errors to JS
#[wasm_bindgen]
pub fn get_error_message(error: VirtualListError) -> String {
    match error {
        VirtualListError::IndexOutOfBounds => "Index out of bounds".to_string(),
        VirtualListError::InvalidSize => "Invalid size provided".to_string(),
        VirtualListError::InvalidViewport => "Invalid viewport size".to_string(),
        VirtualListError::InvalidConfiguration => "Invalid configuration parameters".to_string(),
        VirtualListError::EmptyList => "Operation cannot be performed on an empty list".to_string(),
    }
}

// Define a struct to return visible range results that's compatible with wasm_bindgen
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct VisibleRange {
    start: usize,
    end: usize,
    start_offset: f64,
    end_offset: f64,
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
    
    #[wasm_bindgen(getter)]
    pub fn start_offset(&self) -> f64 {
        self.start_offset
    }
    
    #[wasm_bindgen(getter)]
    pub fn end_offset(&self) -> f64 {
        self.end_offset
    }
}

// Configuration for VirtualList
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct VirtualListConfig {
    buffer_size: usize,
    use_binary_search_in_chunk: bool,
    overscan_items: usize,
    update_batch_size: usize,
}

#[wasm_bindgen]
impl VirtualListConfig {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            buffer_size: 5,
            use_binary_search_in_chunk: true,
            overscan_items: 3,
            update_batch_size: 10,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    #[wasm_bindgen(setter)]
    pub fn set_buffer_size(&mut self, size: usize) {
        self.buffer_size = size;
    }

    #[wasm_bindgen(getter)]
    pub fn use_binary_search_in_chunk(&self) -> bool {
        self.use_binary_search_in_chunk
    }

    #[wasm_bindgen(setter)]
    pub fn set_use_binary_search_in_chunk(&mut self, use_binary: bool) {
        self.use_binary_search_in_chunk = use_binary;
    }
    
    #[wasm_bindgen(getter)]
    pub fn overscan_items(&self) -> usize {
        self.overscan_items
    }
    
    #[wasm_bindgen(setter)]
    pub fn set_overscan_items(&mut self, items: usize) {
        self.overscan_items = items;
    }
    
    #[wasm_bindgen(getter)]
    pub fn update_batch_size(&self) -> usize {
        self.update_batch_size
    }
    
    #[wasm_bindgen(setter)]
    pub fn set_update_batch_size(&mut self, size: usize) {
        self.update_batch_size = size;
    }
}

// Define the Chunk struct to hold item sizes
#[derive(Clone, Debug)]
struct Chunk {
    sizes: Vec<f64>,       // Sizes of items in this chunk
    chunk_total_size: f64, // Cached total size of this chunk
    prefix_sums: Vec<f64>, // Prefix sums for quick position lookups
}

impl Chunk {
    fn new(chunk_size: usize, estimated_size: f64) -> Chunk {
        let estimated_size = estimated_size.max(0.0); // Ensure sizes are non-negative
        let sizes = vec![estimated_size; chunk_size];
        let chunk_total_size = estimated_size * chunk_size as f64;
        
        // Calculate prefix sums for faster position lookups
        let mut prefix_sums = Vec::with_capacity(chunk_size + 1);
        prefix_sums.push(0.0);
        
        let mut sum = 0.0;
        for size in &sizes {
            sum += size;
            prefix_sums.push(sum);
        }

        Chunk {
            sizes,
            chunk_total_size,
            prefix_sums,
        }
    }

    // Get the total size without recomputing
    fn get_total_size(&self) -> f64 {
        self.chunk_total_size
    }
    
    // Get the number of items in this chunk
    fn len(&self) -> usize {
        self.sizes.len()
    }
    
    // Check if chunk is empty
    fn is_empty(&self) -> bool {
        self.sizes.is_empty()
    }

    // Update an item size and the chunk's total size
    fn update_size(&mut self, index: usize, new_size: f64) -> Result<f64, VirtualListError> {
        if index >= self.sizes.len() {
            return Err(VirtualListError::IndexOutOfBounds);
        }

        if new_size < 0.0 {
            return Err(VirtualListError::InvalidSize);
        }

        let old_size = self.sizes[index];
        let size_diff = new_size - old_size;
        self.sizes[index] = new_size;
        self.chunk_total_size += size_diff;
        
        // Update prefix sums from this index forward
        for i in index + 1..=self.sizes.len() {
            self.prefix_sums[i] += size_diff;
        }

        Ok(size_diff)
    }

    // Get the size of an item at a specific index within the chunk
    fn get_size(&self, index: usize) -> Result<f64, VirtualListError> {
        if index >= self.sizes.len() {
            return Err(VirtualListError::IndexOutOfBounds);
        }

        Ok(self.sizes[index])
    }

    // Get the position of an item within the chunk (sum of sizes before it)
    fn get_position(&self, index: usize) -> Result<f64, VirtualListError> {
        if index > self.sizes.len() {
            return Err(VirtualListError::IndexOutOfBounds);
        }

        // Use prefix sums for O(1) lookup
        Ok(self.prefix_sums[index])
    }

    // Find the item at a given position using binary search
    fn find_item_at_position(&self, position: f64, use_binary_search: bool) -> Result<(usize, f64), VirtualListError> {
        if position < 0.0 || (self.is_empty() && position > 0.0) || (!self.is_empty() && position > self.chunk_total_size) {
            return Err(VirtualListError::InvalidSize);
        }
        
        if self.is_empty() {
            return Ok((0, 0.0));
        }

        // Handle edge cases
        if position <= 0.0 {
            return Ok((0, 0.0));
        }
        
        if position >= self.chunk_total_size {
            return Ok((self.sizes.len() - 1, self.chunk_total_size - self.sizes[self.sizes.len() - 1]));
        }

        if use_binary_search {
            // Simplified binary search using prefix sums
            // Find the first prefix sum that's greater than position
            let mut low = 0;
            let mut high = self.prefix_sums.len() - 1;
            
            while low < high {
                let mid = low + (high - low) / 2;
                if self.prefix_sums[mid] < position {
                    low = mid + 1;
                } else {
                    high = mid;
                }
            }
            
            // The item index is one less than the prefix sum index
            let item_index = if low > 0 && self.prefix_sums[low] > position {
                low - 1
            } else {
                low
            };
            
            let offset = position - self.prefix_sums[item_index];
            return Ok((item_index, offset));
        } else {
            // Linear search
            for i in 0..self.sizes.len() {
                let start_pos = self.prefix_sums[i];
                let end_pos = self.prefix_sums[i + 1];
                
                if position >= start_pos && position < end_pos {
                    return Ok((i, position - start_pos));
                }
            }
            
            // Fallback - return last item if somehow not found
            return Ok((self.sizes.len() - 1, position - self.prefix_sums[self.sizes.len() - 1]));
        }
    }
}

// Define the VirtualList struct
#[wasm_bindgen]
pub struct VirtualList {
    total_items: usize,    // Total number of items in the list
    estimated_size: f64,   // Default size for unmeasured items
    orientation: Orientation, // List orientation
    chunks: Vec<Chunk>,    // Chunks of items for efficient management
    chunk_size: usize,     // Number of items per chunk
    cumulative_sizes: Vec<f64>, // Cumulative sizes up to each chunk
    total_size: f64,       // Total size of all items (cached for performance)
    config: VirtualListConfig, // Configuration options
    pending_updates: Vec<(usize, f64)>, // Pending size updates for batch processing
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
        let config = VirtualListConfig::new();
        Self::new_with_config(total_items, chunk_size, estimated_size, orientation, config)
    }

    /// Constructor with custom configuration
    pub fn new_with_config(
        total_items: usize,
        chunk_size: usize,
        estimated_size: f64,
        orientation: Orientation,
        config: VirtualListConfig,
    ) -> VirtualList {
        // Validate inputs
        let chunk_size = cmp::max(1, chunk_size); // Ensure chunk size is at least 1
        let estimated_size = estimated_size.max(0.0); // Ensure estimated size is non-negative

        let num_chunks = (total_items + chunk_size - 1) / chunk_size; // Ceiling division
        let mut chunks = Vec::with_capacity(num_chunks);
        let mut cumulative_sizes = Vec::with_capacity(num_chunks + 1);
        
        // Add a sentinel value at the beginning for easier calculations
        cumulative_sizes.push(0.0);

        let mut running_total = 0.0;
        for i in 0..num_chunks {
            let items_in_chunk = if i == num_chunks - 1 && total_items % chunk_size != 0 {
                // Last chunk might have fewer items
                total_items % chunk_size
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
            config,
            pending_updates: Vec::new(),
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

    /// Get the configuration
    pub fn get_config(&self) -> VirtualListConfig {
        self.config.clone()
    }

    /// Set configuration
    pub fn set_config(&mut self, config: VirtualListConfig) {
        self.config = config;
    }

    /// Set buffer size for visible range calculations
    #[wasm_bindgen]
    pub fn set_buffer_size(&mut self, buffer_size: usize) {
        self.config.set_buffer_size(buffer_size);
    }

    /// Get buffer size
    #[wasm_bindgen]
    pub fn get_buffer_size(&self) -> usize {
        self.config.buffer_size()
    }

    /// Enable or disable binary search in chunks
    #[wasm_bindgen]
    pub fn set_use_binary_search_in_chunk(&mut self, use_binary: bool) {
        self.config.set_use_binary_search_in_chunk(use_binary);
    }

    /// Get current binary search setting
    #[wasm_bindgen]
    pub fn get_use_binary_search_in_chunk(&self) -> bool {
        self.config.use_binary_search_in_chunk()
    }
    
    /// Set overscan items count
    #[wasm_bindgen]
    pub fn set_overscan_items(&mut self, items: usize) {
        self.config.set_overscan_items(items);
    }
    
    /// Get overscan items count
    #[wasm_bindgen]
    pub fn get_overscan_items(&self) -> usize {
        self.config.overscan_items()
    }
    
    /// Set update batch size
    #[wasm_bindgen]
    pub fn set_update_batch_size(&mut self, size: usize) {
        self.config.set_update_batch_size(size);
    }
    
    /// Get update batch size
    #[wasm_bindgen]
    pub fn get_update_batch_size(&self) -> usize {
        self.config.update_batch_size()
    }

    /// Convert internal error to JsValue
    fn convert_error(error: VirtualListError) -> JsValue {
        JsValue::from_str(&get_error_message(error))
    }

    /// Get the position of an item in the list
    #[wasm_bindgen]
    pub fn get_position(&self, index: usize) -> Result<f64, JsValue> {
        if index >= self.total_items {
            return Err(Self::convert_error(VirtualListError::IndexOutOfBounds));
        }

        let chunk_idx = index / self.chunk_size;
        let item_idx_in_chunk = index % self.chunk_size;

        // Position is the cumulative size up to the current chunk plus the size of items in the current chunk up to the index
        let chunk_start = self.cumulative_sizes[chunk_idx];
        
        let chunk = &self.chunks[chunk_idx];
        match chunk.get_position(item_idx_in_chunk) {
            Ok(position_in_chunk) => Ok(chunk_start + position_in_chunk),
            Err(e) => Err(Self::convert_error(e)),
        }
    }

    /// Queue an item size update for batch processing
    #[wasm_bindgen]
    pub fn queue_update_item_size(&mut self, index: usize, new_size: f64) -> Result<(), JsValue> {
        if index >= self.total_items {
            return Err(Self::convert_error(VirtualListError::IndexOutOfBounds));
        }

        if new_size < 0.0 {
            return Err(Self::convert_error(VirtualListError::InvalidSize));
        }
        
        self.pending_updates.push((index, new_size));
        
        // Process batch if we've reached the batch size
        if self.pending_updates.len() >= self.config.update_batch_size() {
            self.process_pending_updates()?;
        }
        
        Ok(())
    }
    
    /// Process all pending updates in one batch
    #[wasm_bindgen]
    pub fn process_pending_updates(&mut self) -> Result<(), JsValue> {
        if self.pending_updates.is_empty() {
            return Ok(());
        }
        
        // Sort updates by index to process them efficiently
        self.pending_updates.sort_by_key(|(idx, _)| *idx);
        
        // Group updates by chunk
        let mut chunk_updates: Vec<Vec<(usize, f64)>> = vec![Vec::new(); self.chunks.len()];
        
        for (index, new_size) in self.pending_updates.drain(..) {
            let chunk_idx = index / self.chunk_size;
            let item_idx_in_chunk = index % self.chunk_size;
            chunk_updates[chunk_idx].push((item_idx_in_chunk, new_size));
        }
        
        // Process updates chunk by chunk and track total size change
        let mut total_size_diff = 0.0;
        
        for (chunk_idx, updates) in chunk_updates.iter().enumerate() {
            if updates.is_empty() {
                continue;
            }
            
            let chunk = &mut self.chunks[chunk_idx];
            let mut chunk_size_diff = 0.0;
            
            for &(item_idx, new_size) in updates {
                match chunk.update_size(item_idx, new_size) {
                    Ok(diff) => chunk_size_diff += diff,
                    Err(e) => return Err(Self::convert_error(e)),
                }
            }
            
            // Update cumulative sizes for all chunks after this one
            if chunk_size_diff != 0.0 {
                total_size_diff += chunk_size_diff;
                for i in chunk_idx + 1..=self.chunks.len() {
                    self.cumulative_sizes[i] += chunk_size_diff;
                }
            }
        }
        
        // Update total size
        self.total_size += total_size_diff;
        
        Ok(())
    }

    /// Update the size of an item and recalculate cumulative sizes (immediate update)
    #[wasm_bindgen]
    pub fn update_item_size(&mut self, index: usize, new_size: f64) -> Result<(), JsValue> {
        if index >= self.total_items {
            return Err(Self::convert_error(VirtualListError::IndexOutOfBounds));
        }

        if new_size < 0.0 {
            return Err(Self::convert_error(VirtualListError::InvalidSize));
        }

        let chunk_idx = index / self.chunk_size;
        let item_idx_in_chunk = index % self.chunk_size;

        // Update the size and get the difference
        let chunk = &mut self.chunks[chunk_idx];
        let size_diff = match chunk.update_size(item_idx_in_chunk, new_size) {
            Ok(diff) => diff,
            Err(e) => return Err(Self::convert_error(e)),
        };

        // If size didn't change, no need to update cumulative sizes
        if size_diff == 0.0 {
            return Ok(());
        }

        // Update total size
        self.total_size += size_diff;

        // Only update cumulative sizes from this chunk onward
        for i in chunk_idx + 1..=self.chunks.len() {
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
            return Err(Self::convert_error(VirtualListError::InvalidViewport));
        }
        
        if self.total_items == 0 {
            return Err(Self::convert_error(VirtualListError::EmptyList));
        }

        // Find the first visible item
        let (start_idx, start_offset) = self.find_item_at_position(scroll_position)?;

        // Find the last visible item
        let end_position = scroll_position + viewport_size;
        let (end_idx, end_offset) = self.find_item_at_position(end_position)?;

        // Add buffer/overscan items for smoother scrolling
        let buffer = self.config.buffer_size();
        let overscan = self.config.overscan_items();
        let start = start_idx.saturating_sub(buffer + overscan);
        let end = cmp::min(end_idx + buffer + overscan + 1, self.total_items);

        Ok(VisibleRange { 
            start, 
            end,
            start_offset,
            end_offset
        })
    }

    /// Find the item and offset at a given position
    fn find_item_at_position(&self, position: f64) -> Result<(usize, f64), JsValue> {
        // Handle edge cases
        if position <= 0.0 {
            return Ok((0, 0.0));
        }
        
        if position >= self.total_size {
            let last_idx = self.total_items.saturating_sub(1);
            return Ok((last_idx, self.total_size));
        }

        // Binary search to find the chunk containing the position
        let (chunk_idx, position_in_chunk) = self.find_chunk_at_position(position)?;
        
        if chunk_idx >= self.chunks.len() {
            return Err(Self::convert_error(VirtualListError::IndexOutOfBounds));
        }

        // Find the item within the chunk
        let chunk = &self.chunks[chunk_idx];
        let use_binary = self.config.use_binary_search_in_chunk();
        
        match chunk.find_item_at_position(position_in_chunk, use_binary) {
            Ok((item_idx_in_chunk, offset)) => {
                let global_idx = chunk_idx * self.chunk_size + item_idx_in_chunk;
                Ok((global_idx, offset))
            },
            Err(e) => Err(Self::convert_error(e)),
        }
    }
    
    /// Find the chunk containing a given position using binary search
    fn find_chunk_at_position(&self, position: f64) -> Result<(usize, f64), JsValue> {
        if self.chunks.is_empty() {
            return Err(Self::convert_error(VirtualListError::EmptyList));
        }
        
        // Binary search to find the chunk
        let mut low = 0;
        let mut high = self.cumulative_sizes.len() - 1;
        
        while low < high {
            let mid = low + (high - low) / 2;
            if self.cumulative_sizes[mid] < position {
                low = mid + 1;
            } else {
                high = mid;
            }
        }
        
        // Adjust to get the chunk that contains the position
        let chunk_idx = if low > 0 && self.cumulative_sizes[low] > position {
            low - 1
        } else {
            low
        };
        
        // Calculate the position within the chunk
        let chunk_start = if chunk_idx > 0 {
            self.cumulative_sizes[chunk_idx - 1]
        } else {
            0.0
        };
        
        let position_in_chunk = position - chunk_start;
        
        Ok((chunk_idx, position_in_chunk))
    }

    /// Get the size of an item at a specific index
    #[wasm_bindgen]
    pub fn get_item_size(&self, index: usize) -> Result<f64, JsValue> {
        if index >= self.total_items {
            return Err(Self::convert_error(VirtualListError::IndexOutOfBounds));
        }

        let chunk_idx = index / self.chunk_size;
        let item_idx_in_chunk = index % self.chunk_size;

        if chunk_idx >= self.chunks.len() {
            return Err(Self::convert_error(VirtualListError::IndexOutOfBounds));
        }

        match self.chunks[chunk_idx].get_size(item_idx_in_chunk) {
            Ok(size) => Ok(size),
            Err(e) => Err(Self::convert_error(e)),
        }
    }
    
    /// Reset all item sizes to the estimated size
    #[wasm_bindgen]
    pub fn reset_item_sizes(&mut self) -> Result<(), JsValue> {
        // Recreate the list with the current parameters but reset all sizes
        let new_list = VirtualList::new_with_config(
            self.total_items,
            self.chunk_size,
            self.estimated_size,
            self.orientation,
            self.config.clone()
        );
        
        self.chunks = new_list.chunks;
        self.cumulative_sizes = new_list.cumulative_sizes;
        self.total_size = new_list.total_size;
        self.pending_updates.clear();
        
        Ok(())
    }
    
    /// Update multiple item sizes at once
    #[wasm_bindgen]
    pub fn update_batch_item_sizes(&mut self, indices: &[usize], sizes: &[f64]) -> Result<(), JsValue> {
        if indices.len() != sizes.len() {
            return Err(Self::convert_error(VirtualListError::InvalidConfiguration));
        }
        
        for i in 0..indices.len() {
            self.queue_update_item_size(indices[i], sizes[i])?;
        }
        
        self.process_pending_updates()
    }
}
