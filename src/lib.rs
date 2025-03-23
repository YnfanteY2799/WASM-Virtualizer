use serde::{Deserialize, Serialize};
use std::cmp;
use std::collections::{HashMap, VecDeque};
use wasm_bindgen::prelude::*;

/// Represents the orientation of the virtual list (vertical or horizontal scrolling).
/// This enum is exposed to JavaScript via WebAssembly.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Orientation {
    /// Vertical scrolling (e.g., a list scrolling up and down).
    Vertical,
    /// Horizontal scrolling (e.g., a list scrolling left and right).
    Horizontal,
}

/// Custom error types for the VirtualList, exposed to JavaScript.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub enum VirtualListError {
    /// Thrown when an index exceeds the list's bounds.
    IndexOutOfBounds,
    /// Thrown when a size is negative or otherwise invalid.
    InvalidSize,
    /// Thrown when the viewport size is non-positive.
    InvalidViewport,
    /// Thrown when configuration parameters are invalid.
    InvalidConfiguration,
    /// Thrown when operations are attempted on an empty list.
    EmptyList,
}

/// Structure to serialize errors to JavaScript.
#[wasm_bindgen]
#[derive(Serialize, Deserialize)]
pub struct JsError {
    kind: String,
    message: String,
}

#[wasm_bindgen]
impl JsError {
    #[wasm_bindgen(constructor)]
    pub fn new(kind: String, message: String) -> JsError {
        JsError { kind, message }
    }

    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }
}

/// Converts a `VirtualListError` into a JavaScript-compatible `JsValue`.
fn convert_error(error: VirtualListError) -> JsValue {
    let kind = match error {
        VirtualListError::IndexOutOfBounds => "IndexOutOfBounds",
        VirtualListError::InvalidSize => "InvalidSize",
        VirtualListError::InvalidViewport => "InvalidViewport",
        VirtualListError::InvalidConfiguration => "InvalidConfiguration",
        VirtualListError::EmptyList => "EmptyList",
    };
    serde_wasm_bindgen::to_value(&JsError::new(kind.to_string(), get_error_message(error))).unwrap()
}

/// Returns a human-readable message for a given `VirtualListError`.
#[wasm_bindgen]
pub fn get_error_message(error: VirtualListError) -> String {
    match error {
        VirtualListError::IndexOutOfBounds => "Index is out of bounds".to_string(),
        VirtualListError::InvalidSize => "Size must be non-negative".to_string(),
        VirtualListError::InvalidViewport => "Viewport size must be positive".to_string(),
        VirtualListError::InvalidConfiguration => {
            "Configuration parameters must be positive".to_string()
        }
        VirtualListError::EmptyList => "List is empty, operation not allowed".to_string(),
    }
}

/// Represents the range of visible items in the list, including offsets for precise positioning.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct VisibleRange {
    start: usize,      // Index of the first visible item.
    end: usize,        // Index of the last visible item (exclusive).
    start_offset: f64, // Offset from the start of the first visible item.
    end_offset: f64,   // Offset from the start of the last visible item.
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

/// Enum representing different cache eviction policies for managing memory usage.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CacheEvictionPolicy {
    /// Least Recently Used: Evicts chunks that haven’t been accessed recently.
    LRU,
    /// Least Frequently Used: Evicts chunks accessed the fewest times.
    LFU,
}

/// Configuration for the `VirtualList`, controlling buffering, caching, and eviction behavior.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct VirtualListConfig {
    buffer_size: usize,         // Extra chunks to load before/after the visible area.
    overscan_items: usize,      // Extra items to render beyond the viewport.
    update_batch_size: usize,   // Number of updates to batch before processing.
    max_cached_chunks: usize,   // Maximum number of chunks to keep in memory.
    cache_eviction_policy: CacheEvictionPolicy, // Policy for evicting chunks from cache.
}

#[wasm_bindgen]
impl VirtualListConfig {
    /// Creates a new configuration with sensible defaults.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            buffer_size: 5,
            overscan_items: 3,
            update_batch_size: 10,
            max_cached_chunks: 100,
            cache_eviction_policy: CacheEvictionPolicy::LRU,
        }
    }

    // Getters and setters with validation to ensure positive values.
    #[wasm_bindgen(getter)]
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }

    #[wasm_bindgen(setter)]
    pub fn set_buffer_size(&mut self, size: usize) {
        self.buffer_size = size.max(1);
    }

    #[wasm_bindgen(getter)]
    pub fn overscan_items(&self) -> usize {
        self.overscan_items
    }

    #[wasm_bindgen(setter)]
    pub fn set_overscan_items(&mut self, items: usize) {
        self.overscan_items = items.max(1);
    }

    #[wasm_bindgen(getter)]
    pub fn update_batch_size(&self) -> usize {
        self.update_batch_size
    }

    #[wasm_bindgen(setter)]
    pub fn set_update_batch_size(&mut self, size: usize) {
        self.update_batch_size = size.max(1);
    }

    #[wasm_bindgen(getter)]
    pub fn max_cached_chunks(&self) -> usize {
        self.max_cached_chunks
    }

    #[wasm_bindgen(setter)]
    pub fn set_max_cached_chunks(&mut self, size: usize) {
        self.max_cached_chunks = size.max(1);
    }

    #[wasm_bindgen(getter)]
    pub fn cache_eviction_policy(&self) -> CacheEvictionPolicy {
        self.cache_eviction_policy
    }

    #[wasm_bindgen(setter)]
    pub fn set_cache_eviction_policy(&mut self, policy: CacheEvictionPolicy) {
        self.cache_eviction_policy = policy;
    }
}

/// Represents a single chunk of items in the virtual list, managing item sizes and positions.
#[derive(Clone, Debug)]
struct Chunk {
    sizes: Vec<f64>,       // Size of each item in the chunk.
    prefix_sums: Vec<f64>, // Cumulative size up to each item, including a 0 at the start.
    total_size: f64,       // Total size of all items in the chunk.
}

impl Chunk {
    /// Creates a new chunk with a specified number of items and an estimated size per item.
    fn new(chunk_size: usize, estimated_size: f64) -> Chunk {
        let estimated_size = estimated_size.max(0.0);
        let sizes = vec![estimated_size; chunk_size];
        let mut prefix_sums = Vec::with_capacity(chunk_size + 1);
        prefix_sums.push(0.0);
        let mut total_size = 0.0;

        for &size in &sizes {
            total_size += size;
            prefix_sums.push(total_size);
        }

        Chunk {
            sizes,
            prefix_sums,
            total_size,
        }
    }

    /// Returns the number of items in the chunk.
    fn len(&self) -> usize {
        self.sizes.len()
    }

    /// Updates the size of an item and adjusts the total size and prefix sums accordingly.
    fn update_size(&mut self, index: usize, new_size: f64) -> Result<f64, VirtualListError> {
        if index >= self.sizes.len() {
            return Err(VirtualListError::IndexOutOfBounds);
        }
        if new_size < 0.0 {
            return Err(VirtualListError::InvalidSize);
        }
        let old_size = self.sizes[index];
        let diff = new_size - old_size;
        self.sizes[index] = new_size;
        self.total_size += diff;
        for i in index + 1..self.prefix_sums.len() {
            self.prefix_sums[i] += diff;
        }
        Ok(diff)
    }

    /// Gets the size of an item at the specified index.
    fn get_size(&self, index: usize) -> Result<f64, VirtualListError> {
        self.sizes
            .get(index)
            .copied()
            .ok_or(VirtualListError::IndexOutOfBounds)
    }

    /// Finds the item at a given position within the chunk using binary search.
    fn find_item_at_position(&self, position: f64) -> Result<(usize, f64), VirtualListError> {
        if position < 0.0 || position > self.total_size {
            return Err(VirtualListError::InvalidSize);
        }
        if self.sizes.is_empty() {
            return Ok((0, 0.0));
        }
        let index = self
            .prefix_sums
            .binary_search_by(|&sum| sum.partial_cmp(&position).unwrap())
            .unwrap_or_else(|e| e - 1);
        let offset = position - self.prefix_sums[index];
        Ok((index, offset))
    }
}

/// Manages cache eviction based on the selected policy (LRU or LFU).
struct CacheEvictionManager {
    policy: CacheEvictionPolicy,       // Eviction policy to use.
    lru_order: VecDeque<usize>,        // Tracks chunk access order for LRU.
    frequency: HashMap<usize, usize>,  // Tracks chunk access frequency for LFU.
}

impl CacheEvictionManager {
    /// Creates a new cache eviction manager with the specified policy.
    fn new(policy: CacheEvictionPolicy) -> Self {
        Self {
            policy,
            lru_order: VecDeque::new(),
            frequency: HashMap::new(),
        }
    }

    /// Updates access tracking when a chunk is accessed.
    fn access(&mut self, chunk_idx: usize) {
        match self.policy {
            CacheEvictionPolicy::LRU => {
                if let Some(pos) = self.lru_order.iter().position(|&x| x == chunk_idx) {
                    self.lru_order.remove(pos);
                }
                self.lru_order.push_back(chunk_idx);
            }
            CacheEvictionPolicy::LFU => {
                *self.frequency.entry(chunk_idx).or_insert(0) += 1;
            }
        }
    }

    /// Evicts a chunk based on the policy and returns its index.
    fn evict(&mut self) -> Option<usize> {
        match self.policy {
            CacheEvictionPolicy::LRU => self.lru_order.pop_front(),
            CacheEvictionPolicy::LFU => self
                .frequency
                .iter()
                .min_by_key(|&(_, &freq)| freq)
                .map(|(&chunk_idx, _)| {
                    self.frequency.remove(&chunk_idx);
                    chunk_idx
                }),
        }
    }
}

/// A virtual list implementation for efficiently rendering large lists in a web environment.
/// Manages items in chunks, supports dynamic sizing, and integrates with WebAssembly.
#[wasm_bindgen]
pub struct VirtualList {
    total_items: usize,              // Total number of items in the list.
    estimated_size: f64,             // Default size for items without explicit sizes.
    orientation: Orientation,        // Scroll direction (vertical or horizontal).
    chunks: HashMap<usize, Chunk>,   // Loaded chunks of items.
    chunk_size: usize,               // Number of items per chunk.
    cumulative_sizes: HashMap<usize, f64>, // Cumulative size up to each chunk’s end.
    total_size: f64,                 // Total size of the entire list.
    config: VirtualListConfig,       // Configuration settings for buffering and caching.
    pending_updates: Vec<(usize, f64)>, // Queued size updates to process in batches.
    cache_eviction_manager: CacheEvictionManager, // Manages chunk eviction from cache.
}

#[wasm_bindgen]
impl VirtualList {
    /// Creates a new `VirtualList` with default configuration.
    #[wasm_bindgen(constructor)]
    pub fn new(
        total_items: usize,
        chunk_size: usize,
        estimated_size: f64,
        orientation: Orientation,
    ) -> VirtualList {
        Self::new_with_config(
            total_items,
            chunk_size,
            estimated_size,
            orientation,
            VirtualListConfig::new(),
        )
    }

    /// Creates a new `VirtualList` with custom configuration.
    pub fn new_with_config(
        total_items: usize,
        chunk_size: usize,
        estimated_size: f64,
        orientation: Orientation,
        config: VirtualListConfig,
    ) -> VirtualList {
        if chunk_size == 0
            || config.buffer_size == 0
            || config.overscan_items == 0
            || config.update_batch_size == 0
            || config.max_cached_chunks == 0
        {
            panic!("All size parameters must be positive");
        }
        let estimated_size = estimated_size.max(0.0);
        let total_size = estimated_size * total_items as f64;
        let cache_eviction_manager = CacheEvictionManager::new(config.cache_eviction_policy);

        VirtualList {
            total_items,
            estimated_size,
            orientation,
            chunks: HashMap::new(),
            chunk_size,
            cumulative_sizes: HashMap::new(),
            total_size,
            config,
            pending_updates: Vec::new(),
            cache_eviction_manager,
        }
    }

    /// Retrieves or creates a chunk at the specified index, managing cache limits.
    fn get_or_create_chunk(&mut self, chunk_idx: usize) -> Result<&mut Chunk, VirtualListError> {
        let num_chunks = (self.total_items + self.chunk_size - 1) / self.chunk_size;
        if chunk_idx >= num_chunks {
            return Err(VirtualListError::IndexOutOfBounds);
        }

        if !self.chunks.contains_key(&chunk_idx) {
            let items_in_chunk =
                if chunk_idx == num_chunks - 1 && self.total_items % self.chunk_size != 0 {
                    self.total_items % self.chunk_size
                } else {
                    self.chunk_size
                };
            self.chunks
                .insert(chunk_idx, Chunk::new(items_in_chunk, self.estimated_size));
            self.update_cumulative_sizes_from(chunk_idx)?;
            self.cache_eviction_manager.access(chunk_idx);

            while self.chunks.len() > self.config.max_cached_chunks {
                if let Some(old_idx) = self.cache_eviction_manager.evict() {
                    self.chunks.remove(&old_idx);
                    self.cumulative_sizes.remove(&old_idx);
                }
            }
        } else {
            self.cache_eviction_manager.access(chunk_idx);
        }
        Ok(self.chunks.get_mut(&chunk_idx).expect("Chunk should exist"))
    }

    /// Updates cumulative sizes starting from a given chunk index, avoiding multiple mutable borrows.
    fn update_cumulative_sizes_from(&mut self, from_chunk: usize) -> Result<(), VirtualListError> {
        let num_chunks = (self.total_items + self.chunk_size - 1) / self.chunk_size;
        if from_chunk >= num_chunks {
            return Err(VirtualListError::IndexOutOfBounds);
        }

        // Compute updates in a local scope to avoid borrowing issues.
        let mut updates = Vec::new();
        let mut cumulative = if from_chunk == 0 {
            0.0
        } else {
            *self.cumulative_sizes.get(&(from_chunk - 1)).unwrap_or(&0.0)
        };

        for i in from_chunk..num_chunks {
            cumulative += if let Some(chunk) = self.chunks.get(&i) {
                chunk.total_size
            } else {
                let items = if i == num_chunks - 1 && self.total_items % self.chunk_size != 0 {
                    self.total_items % self.chunk_size
                } else {
                    self.chunk_size
                };
                self.estimated_size * items as f64
            };
            updates.push((i, cumulative));
        }

        // Apply all updates in a single mutable borrow.
        for (i, size) in updates {
            self.cumulative_sizes.insert(i, size);
        }
        Ok(())
    }

    /// Returns the total size of the list.
    #[wasm_bindgen]
    pub fn get_total_size(&self) -> f64 {
        self.total_size
    }

    /// Returns the total number of items in the list.
    #[wasm_bindgen]
    pub fn get_total_items(&self) -> usize {
        self.total_items
    }

    /// Gets the size of an item at the specified index.
    #[wasm_bindgen]
    pub fn get_item_size(&self, index: usize) -> Result<f64, JsValue> {
        if index >= self.total_items {
            return Err(convert_error(VirtualListError::IndexOutOfBounds));
        }
        let chunk_idx = index / self.chunk_size;
        let item_idx = index % self.chunk_size;
        if let Some(chunk) = self.chunks.get(&chunk_idx) {
            chunk.get_size(item_idx).map_err(convert_error)
        } else {
            Ok(self.estimated_size)
        }
    }

    /// Updates the size of an item at the specified index and adjusts related data.
    #[wasm_bindgen]
    pub fn update_item_size(&mut self, index: usize, new_size: f64) -> Result<(), JsValue> {
        if index >= self.total_items {
            return Err(convert_error(VirtualListError::IndexOutOfBounds));
        }
        let chunk_idx = index / self.chunk_size;
        let item_idx = index % self.chunk_size;
        let chunk = self.get_or_create_chunk(chunk_idx).map_err(convert_error)?;
        let diff = chunk
            .update_size(item_idx, new_size)
            .map_err(convert_error)?;
        self.total_size += diff;
        self.update_cumulative_sizes_from(chunk_idx)
            .map_err(convert_error)?;
        Ok(())
    }

    /// Returns the range of visible items based on scroll position and viewport size.
    #[wasm_bindgen]
    pub fn get_visible_range(
        &mut self,
        scroll_position: f64,
        viewport_size: f64,
    ) -> Result<VisibleRange, JsValue> {
        if viewport_size <= 0.0 {
            return Err(convert_error(VirtualListError::InvalidViewport));
        }
        if self.total_items == 0 {
            return Err(convert_error(VirtualListError::EmptyList));
        }

        let scroll_position = scroll_position.max(0.0).min(self.total_size);
        let end_position = (scroll_position + viewport_size).min(self.total_size);

        let (start_idx, start_offset) = self.find_item_at_position(scroll_position)?;
        let (end_idx, end_offset) = self.find_item_at_position(end_position)?;

        let buffer = self.config.buffer_size;
        let overscan = self.config.overscan_items;
        let start = start_idx.saturating_sub(buffer * self.chunk_size + overscan);
        let end = cmp::min(end_idx + buffer * self.chunk_size + overscan + 1, self.total_items);

        Ok(VisibleRange {
            start,
            end,
            start_offset,
            end_offset,
        })
    }

    /// Finds the chunk containing the given position using binary search.
    fn find_chunk_at_position(&self, position: f64) -> Result<(usize, f64), VirtualListError> {
        if self.total_items == 0 {
            return Err(VirtualListError::EmptyList);
        }
        let num_chunks = (self.total_items + self.chunk_size - 1) / self.chunk_size;
        let position = position.max(0.0).min(self.total_size);

        let mut low = 0;
        let mut high = num_chunks;
        let mut last_cumulative = 0.0;

        while low < high {
            let mid = low + (high - low) / 2;
            let cumulative = self.cumulative_sizes.get(&mid).copied().unwrap_or_else(|| {
                let items = if mid == num_chunks - 1 && self.total_items % self.chunk_size != 0 {
                    self.total_items % self.chunk_size
                } else {
                    self.chunk_size
                };
                (mid as f64 * self.chunk_size as f64 * self.estimated_size).min(self.total_size)
            });

            if cumulative <= position {
                low = mid + 1;
                last_cumulative = cumulative;
            } else {
                high = mid;
            }
        }

        let chunk_idx = if low > 0 { low - 1 } else { 0 };
        let position_in_chunk = position - last_cumulative;
        Ok((chunk_idx, position_in_chunk))
    }

    /// Finds the item at a given position in the list, loading chunks as needed.
    fn find_item_at_position(&mut self, position: f64) -> Result<(usize, f64), JsValue> {
        if self.total_items == 0 {
            return Ok((0, 0.0));
        }
        let position = position.max(0.0).min(self.total_size);
        let (chunk_idx, position_in_chunk) = self
            .find_chunk_at_position(position)
            .map_err(convert_error)?;
        let chunk = self.get_or_create_chunk(chunk_idx).map_err(convert_error)?;
        let (item_idx, offset) = chunk.find_item_at_position(position_in_chunk)?;
        let global_idx = chunk_idx * self.chunk_size + item_idx;
        Ok((global_idx.min(self.total_items - 1), offset))
    }

    /// Queues an item size update to be processed later in a batch.
    #[wasm_bindgen]
    pub fn queue_update_item_size(&mut self, index: usize, new_size: f64) -> Result<(), JsValue> {
        if index >= self.total_items {
            return Err(convert_error(VirtualListError::IndexOutOfBounds));
        }
        if new_size < 0.0 {
            return Err(convert_error(VirtualListError::InvalidSize));
        }
        self.pending_updates.push((index, new_size));
        if self.pending_updates.len() >= self.config.update_batch_size {
            self.process_pending_updates()?;
        }
        Ok(())
    }

    /// Processes all queued size updates in a batch, minimizing borrow conflicts.
    #[wasm_bindgen]
    pub fn process_pending_updates(&mut self) -> Result<(), JsValue> {
        if self.pending_updates.is_empty() {
            return Ok(());
        }

        let mut chunk_updates: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
        for (index, size) in self.pending_updates.drain(..) {
            let chunk_idx = index / self.chunk_size;
            let item_idx = index % self.chunk_size;
            chunk_updates
                .entry(chunk_idx)
                .or_default()
                .push((item_idx, size));
        }

        let mut total_diff = 0.0;
        let mut min_chunk_idx = usize::MAX;
        for (chunk_idx, updates) in chunk_updates {
            let chunk = self.get_or_create_chunk(chunk_idx).map_err(convert_error)?;
            for (item_idx, new_size) in updates {
                total_diff += chunk
                    .update_size(item_idx, new_size)
                    .map_err(convert_error)?;
            }
            min_chunk_idx = min_chunk_idx.min(chunk_idx);
        }
        self.total_size += total_diff;
        if min_chunk_idx != usize::MAX {
            self.update_cumulative_sizes_from(min_chunk_idx)
                .map_err(convert_error)?;
        }
        Ok(())
    }
}