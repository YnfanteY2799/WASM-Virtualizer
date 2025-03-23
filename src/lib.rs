use serde::{Deserialize, Serialize};
use std::cmp;
use std::collections::{HashMap, VecDeque, HashSet};
use wasm_bindgen::prelude::*;

/// Represents the orientation of the virtual list (vertical or horizontal scrolling).
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

/// Custom error types for the VirtualList, exposed to JavaScript.
#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub enum VirtualListError {
    IndexOutOfBounds,
    InvalidSize,
    InvalidViewport,
    InvalidConfiguration,
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
    pub fn new(kind: &str, message: &str) -> JsError {
        JsError {
            kind: kind.to_string(),
            message: message.to_string(),
        }
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

/// Converts a VirtualListError to a JavaScript-compatible value.
fn convert_error(error: VirtualListError) -> JsValue {
    let kind = match error {
        VirtualListError::IndexOutOfBounds => "IndexOutOfBounds",
        VirtualListError::InvalidSize => "InvalidSize",
        VirtualListError::InvalidViewport => "InvalidViewport",
        VirtualListError::InvalidConfiguration => "InvalidConfiguration",
        VirtualListError::EmptyList => "EmptyList",
    };
    serde_wasm_bindgen::to_value(&JsError::new(kind, &get_error_message(error))).unwrap()
}

/// Returns a human-readable error message for a given error.
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

/// Represents the range of visible items in the list, including offsets.
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

/// Enum representing different cache eviction policies.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CacheEvictionPolicy {
    LRU,
    LFU,
}

/// Configuration for the VirtualList, controlling buffering, caching, and eviction behavior.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct VirtualListConfig {
    buffer_size: usize,
    overscan_items: usize,
    update_batch_size: usize,
    max_cached_chunks: usize,
    cache_eviction_policy: CacheEvictionPolicy,
}

#[wasm_bindgen]
impl VirtualListConfig {
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

/// Represents a single chunk of items in the virtual list.
#[derive(Clone, Debug)]
struct Chunk {
    sizes: Vec<f64>,
    prefix_sums: Vec<f64>,
    total_size: f64,
}

impl Chunk {
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

    fn get_size(&self, index: usize) -> Result<f64, VirtualListError> {
        self.sizes
            .get(index)
            .copied()
            .ok_or(VirtualListError::IndexOutOfBounds)
    }

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

/// Manages cache eviction based on the selected policy.
struct CacheEvictionManager {
    policy: CacheEvictionPolicy,
    lru_order: VecDeque<usize>,
    lru_set: HashSet<usize>,
    frequency: HashMap<usize, usize>,
}

impl CacheEvictionManager {
    fn new(policy: CacheEvictionPolicy) -> Self {
        Self {
            policy,
            lru_order: VecDeque::new(),
            lru_set: HashSet::new(),
            frequency: HashMap::new(),
        }
    }

    fn access(&mut self, chunk_idx: usize) {
        match self.policy {
            CacheEvictionPolicy::LRU => {
                if self.lru_set.contains(&chunk_idx) {
                    self.lru_order.retain(|&x| x != chunk_idx);
                }
                self.lru_order.push_back(chunk_idx);
                self.lru_set.insert(chunk_idx);
            }
            CacheEvictionPolicy::LFU => {
                *self.frequency.entry(chunk_idx).or_insert(0) += 1;
            }
        }
    }

    fn evict(&mut self) -> Option<usize> {
        match self.policy {
            CacheEvictionPolicy::LRU => {
                if let Some(chunk_idx) = self.lru_order.pop_front() {
                    self.lru_set.remove(&chunk_idx);
                    Some(chunk_idx)
                } else {
                    None
                }
            }
            CacheEvictionPolicy::LFU => {
                let min_freq_entry = self.frequency.iter().min_by_key(|&(_, &freq)| freq);
                if let Some((&chunk_idx, _)) = min_freq_entry {
                    self.frequency.remove(&chunk_idx);
                    Some(chunk_idx)
                } else {
                    None
                }
            }
        }
    }
}

/// A virtual list implementation for efficiently rendering large lists in a web environment.
#[wasm_bindgen]
pub struct VirtualList {
    total_items: usize,
    estimated_size: f64,
    #[allow(dead_code)]
    orientation: Orientation,
    chunks: Vec<Option<Chunk>>,
    chunk_size: usize,
    cumulative_sizes: Vec<f64>,
    total_size: f64,
    config: VirtualListConfig,
    pending_updates: Vec<(usize, f64)>,
    cache_eviction_manager: CacheEvictionManager,
}

#[wasm_bindgen]
impl VirtualList {
    /// Creates a new VirtualList with default configuration.
    ///
    /// # Arguments
    /// * `total_items` - Total number of items in the list.
    /// * `chunk_size` - Number of items per chunk.
    /// * `estimated_size` - Estimated size of each item.
    /// * `orientation` - Orientation of the list (Vertical or Horizontal).
    ///
    /// # Returns
    /// A `Result` containing the `VirtualList` or a `JsValue` error if the configuration is invalid.
    ///
    /// # Examples
    /// ```javascript
    /// let list = VirtualList.new(1000, 50, 20.0, Orientation.Vertical);
    /// ```
    #[wasm_bindgen(constructor)]
    pub fn new(
        total_items: usize,
        chunk_size: usize,
        estimated_size: f64,
        orientation: Orientation,
    ) -> Result<VirtualList, JsValue> {
        Self::new_with_config(
            total_items,
            chunk_size,
            estimated_size,
            orientation,
            VirtualListConfig::new(),
        )
    }

    /// Creates a new VirtualList with custom configuration.
    ///
    /// # Arguments
    /// * `total_items` - Total number of items in the list.
    /// * `chunk_size` - Number of items per chunk.
    /// * `estimated_size` - Estimated size of each item.
    /// * `orientation` - Orientation of the list (Vertical or Horizontal).
    /// * `config` - Custom configuration for buffering and caching.
    ///
    /// # Returns
    /// A `Result` containing the `VirtualList` or a `JsValue` error if the configuration is invalid.
    pub fn new_with_config(
        total_items: usize,
        chunk_size: usize,
        estimated_size: f64,
        orientation: Orientation,
        config: VirtualListConfig,
    ) -> Result<VirtualList, JsValue> {
        if chunk_size == 0
            || config.buffer_size == 0
            || config.overscan_items == 0
            || config.update_batch_size == 0
            || config.max_cached_chunks == 0
        {
            return Err(convert_error(VirtualListError::InvalidConfiguration));
        }
        let estimated_size = estimated_size.max(0.0);
        let num_chunks = (total_items + chunk_size - 1) / chunk_size;
        let total_size = estimated_size * total_items as f64;
        let cache_eviction_manager = CacheEvictionManager::new(config.cache_eviction_policy);
        let update_batch_size = config.update_batch_size; // Extract before move

        Ok(VirtualList {
            total_items,
            estimated_size,
            orientation,
            chunks: vec![None; num_chunks],
            chunk_size,
            cumulative_sizes: vec![0.0; num_chunks],
            total_size,
            config, // Move occurs here
            pending_updates: Vec::with_capacity(update_batch_size), // Use extracted value
            cache_eviction_manager,
        })
    }

    /// Retrieves or creates a chunk at the given index, managing cache eviction if necessary.
    fn get_or_create_chunk(&mut self, chunk_idx: usize) -> Result<&mut Chunk, VirtualListError> {
        if chunk_idx >= self.chunks.len() {
            return Err(VirtualListError::IndexOutOfBounds);
        }

        if self.chunks[chunk_idx].is_none() {
            let items_in_chunk = if chunk_idx == self.chunks.len() - 1 && self.total_items % self.chunk_size != 0 {
                self.total_items % self.chunk_size
            } else {
                self.chunk_size
            };
            self.chunks[chunk_idx] = Some(Chunk::new(items_in_chunk, self.estimated_size));
            self.update_cumulative_sizes_from(chunk_idx)?;
            self.cache_eviction_manager.access(chunk_idx);

            while self.chunks.iter().filter(|c| c.is_some()).count() > self.config.max_cached_chunks {
                if let Some(old_idx) = self.cache_eviction_manager.evict() {
                    self.chunks[old_idx] = None;
                }
            }
        } else {
            self.cache_eviction_manager.access(chunk_idx);
        }
        Ok(self.chunks[chunk_idx].as_mut().expect("Chunk should exist"))
    }

    /// Updates cumulative sizes starting from a given chunk index.
    fn update_cumulative_sizes_from(&mut self, from_chunk: usize) -> Result<(), VirtualListError> {
        let num_chunks = self.chunks.len();
        if from_chunk >= num_chunks {
            return Err(VirtualListError::IndexOutOfBounds);
        }

        let mut cumulative = if from_chunk == 0 {
            0.0
        } else {
            self.cumulative_sizes[from_chunk - 1]
        };

        for i in from_chunk..num_chunks {
            cumulative += if let Some(chunk) = &self.chunks[i] {
                chunk.total_size
            } else {
                let items = if i == num_chunks - 1 && self.total_items % self.chunk_size != 0 {
                    self.total_items % self.chunk_size
                } else {
                    self.chunk_size
                };
                self.estimated_size * items as f64
            };
            self.cumulative_sizes[i] = cumulative;
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
        if let Some(chunk) = &self.chunks[chunk_idx] {
            chunk.get_size(item_idx).map_err(convert_error)
        } else {
            Ok(self.estimated_size)
        }
    }

    /// Updates the size of an item at the specified index.
    ///
    /// # Arguments
    /// * `index` - Index of the item to update.
    /// * `new_size` - New size of the item.
    ///
    /// # Returns
    /// A `Result` indicating success or a `JsValue` error if the index or size is invalid.
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
    ///
    /// # Arguments
    /// * `scroll_position` - Current scroll position.
    /// * `viewport_size` - Size of the viewport.
    ///
    /// # Returns
    /// A `Result` containing the `VisibleRange` or a `JsValue` error if the input is invalid.
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
        let start = start_idx.saturating_sub(buffer + overscan);
        let end = cmp::min(end_idx + buffer + overscan + 1, self.total_items);

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
        let position = position.max(0.0).min(self.total_size);

        // Binary search on cumulative_sizes
        let chunk_idx = self.cumulative_sizes.binary_search_by(|&sum| sum.partial_cmp(&position).unwrap())
            .unwrap_or_else(|e| e - 1);

        let last_cumulative = if chunk_idx == 0 {
            0.0
        } else {
            self.cumulative_sizes[chunk_idx - 1]
        };
        let position_in_chunk = position - last_cumulative;
        Ok((chunk_idx, position_in_chunk))
    }

    /// Finds the item at a given position in the list.
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

    /// Processes all queued size updates in a batch.
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