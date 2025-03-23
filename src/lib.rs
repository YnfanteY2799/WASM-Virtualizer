use wasm_bindgen::prelude::*;
use serde::Serialize;
use std::cmp;
use std::collections::{HashMap, VecDeque, HashSet, BinaryHeap};
use std::cmp::Reverse;
use rayon::prelude::*;

// Internal error type
#[derive(Debug)]
enum InternalError {
    IndexOutOfBounds { index: usize, max: usize },
    InvalidSize { value: f64 },
    InvalidViewport { size: f64 },
    InvalidConfiguration { param: &'static str },
    EmptyList,
    PrecisionLimitExceeded,
    InvalidOperation { message: String },
}

// JS error struct
#[derive(Serialize)]
struct JsError {
    kind: String,
    message: String,
}

impl JsError {
    fn new(kind: &str, message: &str) -> Self {
        JsError {
            kind: kind.to_string(),
            message: message.to_string(),
        }
    }
}

// Convert InternalError to JsValue
fn convert_internal_error(error: InternalError) -> JsValue {
    match error {
        InternalError::IndexOutOfBounds { index, max } => convert_error(
            "IndexOutOfBounds",
            &format!("Index {} exceeds maximum {}", index, max),
        ),
        InternalError::InvalidSize { value } => convert_error(
            "InvalidSize",
            &format!("Invalid size: {}", value),
        ),
        InternalError::InvalidViewport { size } => convert_error(
            "InvalidViewport",
            &format!("Invalid viewport size: {}", size),
        ),
        InternalError::InvalidConfiguration { param } => convert_error(
            "InvalidConfiguration",
            &format!("Invalid configuration parameter: {}", param),
        ),
        InternalError::EmptyList => convert_error("EmptyList", "List is empty"),
        InternalError::PrecisionLimitExceeded => convert_error(
            "PrecisionLimitExceeded",
            "Position exceeds safe precision limit",
        ),
        InternalError::InvalidOperation { message } => convert_error("InvalidOperation", &message),
    }
}

// Generic error conversion
fn convert_error(kind: &str, message: &str) -> JsValue {
    serde_wasm_bindgen::to_value(&JsError::new(kind, message)).unwrap()
}

// Orientation enum
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

// Cache eviction policy enum
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum CacheEvictionPolicy {
    LRU,
    LFU,
}

// Configuration struct
#[wasm_bindgen]
#[derive(Clone)]
pub struct VirtualListConfig {
    buffer_size: usize,
    overscan_items: usize,
    update_batch_size: usize,
    max_cached_chunks: usize,
    cache_eviction_policy: CacheEvictionPolicy,
    max_memory_bytes: Option<usize>,
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
            max_memory_bytes: None,
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
    // Add other getters/setters as needed
}

// Visible range struct
#[wasm_bindgen]
#[derive(Clone)]
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

// Chunk struct
#[derive(Debug)]
struct Chunk {
    sizes: Vec<f64>,
    prefix_sums: Vec<f64>,
    total_size: f64,
}

impl Chunk {
    fn new(chunk_size: usize, estimated_size: f64) -> Result<Self, InternalError> {
        if estimated_size.is_nan() || estimated_size < 0.0 {
            return Err(InternalError::InvalidSize { value: estimated_size });
        }
        let sizes = vec![estimated_size; chunk_size];
        let mut prefix_sums = Vec::with_capacity(chunk_size + 1);
        prefix_sums.push(0.0);
        let mut total_size = 0.0;
        for &size in &sizes {
            total_size += size;
            prefix_sums.push(total_size);
        }
        Ok(Chunk {
            sizes,
            prefix_sums,
            total_size,
        })
    }

    fn update_size(&mut self, index: usize, new_size: f64) -> Result<f64, InternalError> {
        if index >= self.sizes.len() {
            return Err(InternalError::IndexOutOfBounds {
                index,
                max: self.sizes.len() - 1,
            });
        }
        if new_size.is_nan() || new_size < 0.0 {
            return Err(InternalError::InvalidSize { value: new_size });
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

    fn get_size(&self, index: usize) -> Result<f64, InternalError> {
        self.sizes
            .get(index)
            .copied()
            .ok_or(InternalError::IndexOutOfBounds {
                index,
                max: self.sizes.len() - 1,
            })
    }

    fn find_item_at_position(&self, position: f64) -> Result<(usize, f64), InternalError> {
        if position.is_nan() || position < 0.0 || position > self.total_size {
            return Err(InternalError::InvalidSize { value: position });
        }
        let index = self
            .prefix_sums
            .binary_search_by(|&sum| {
                sum.partial_cmp(&position).unwrap_or(std::cmp::Ordering::Greater)
            })
            .unwrap_or_else(|e| e - 1);
        let offset = position - self.prefix_sums[index];
        Ok((index, offset))
    }

    fn memory_usage(&self) -> usize {
        (self.sizes.capacity() * 8) + (self.prefix_sums.capacity() * 8)
    }
}

// Cache eviction manager with min-heap for LFU
struct CacheEvictionManager {
    policy: CacheEvictionPolicy,
    lru_order: VecDeque<usize>,
    lru_set: HashSet<usize>,
    frequency: HashMap<usize, usize>,
    min_heap: BinaryHeap<Reverse<(usize, usize)>>, // (frequency, chunk_idx)
}

impl CacheEvictionManager {
    fn new(policy: CacheEvictionPolicy) -> Self {
        Self {
            policy,
            lru_order: VecDeque::new(),
            lru_set: HashSet::new(),
            frequency: HashMap::new(),
            min_heap: BinaryHeap::new(),
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
                let freq = self.frequency.entry(chunk_idx).or_insert(0);
                *freq += 1;
                self.min_heap.push(Reverse((*freq, chunk_idx)));
            }
        }
    }

    fn evict(&mut self) -> Option<usize> {
        match self.policy {
            CacheEvictionPolicy::LRU => {
                self.lru_order.pop_front().map(|idx| {
                    self.lru_set.remove(&idx);
                    idx
                })
            }
            CacheEvictionPolicy::LFU => {
                while let Some(Reverse((freq, idx))) = self.min_heap.pop() {
                    if self.frequency.get(&idx) == Some(&freq) {
                        self.frequency.remove(&idx);
                        return Some(idx);
                    }
                }
                None
            }
        }
    }
}

// Main VirtualList struct
#[wasm_bindgen]
pub struct VirtualList {
    total_items: usize,
    estimated_size: f64,
    orientation: Orientation,
    chunks: Vec<Option<Chunk>>,
    chunk_size: usize,
    cumulative_sizes: Vec<f64>,
    total_size: f64,
    config: VirtualListConfig,
    pending_updates: Vec<(usize, f64)>,
    cache_eviction_manager: CacheEvictionManager,
    current_memory_usage: usize,
}

#[wasm_bindgen]
impl VirtualList {
    #[wasm_bindgen(constructor)]
    pub fn new_with_config(
        total_items: usize,
        chunk_size: usize,
        estimated_size: f64,
        orientation: Orientation,
        config: VirtualListConfig,
    ) -> Result<VirtualList, JsValue> {
        if chunk_size == 0 {
            return Err(convert_error("InvalidConfiguration", "chunk_size must be positive"));
        }
        if config.buffer_size == 0 {
            return Err(convert_error("InvalidConfiguration", "buffer_size must be positive"));
        }
        if estimated_size.is_nan() || estimated_size < 0.0 {
            return Err(convert_error(
                "InvalidSize",
                &format!("Estimated size must be non-negative, got {}", estimated_size),
            ));
        }

        let num_chunks = (total_items + chunk_size - 1) / chunk_size;
        let mut list = VirtualList {
            total_items,
            estimated_size,
            orientation,
            chunks: vec![None; num_chunks],
            chunk_size,
            cumulative_sizes: vec![0.0; num_chunks],
            total_size: estimated_size * total_items as f64,
            config,
            pending_updates: Vec::with_capacity(config.update_batch_size),
            cache_eviction_manager: CacheEvictionManager::new(config.cache_eviction_policy),
            current_memory_usage: 0,
        };
        list.update_cumulative_sizes_from(0).map_err(convert_internal_error)?;
        Ok(list)
    }

    fn get_or_create_chunk(&mut self, chunk_idx: usize) -> Result<&mut Chunk, InternalError> {
        if chunk_idx >= self.chunks.len() {
            return Err(InternalError::IndexOutOfBounds {
                index: chunk_idx,
                max: self.chunks.len() - 1,
            });
        }
        if self.chunks[chunk_idx].is_none() {
            let items_in_chunk = if chunk_idx == self.chunks.len() - 1
                && self.total_items % self.chunk_size != 0
            {
                self.total_items % self.chunk_size
            } else {
                self.chunk_size
            };
            let chunk = Chunk::new(items_in_chunk, self.estimated_size)?;
            self.current_memory_usage += chunk.memory_usage();
            self.chunks[chunk_idx] = Some(chunk);
            self.cache_eviction_manager.access(chunk_idx);
            self.enforce_memory_limit()?;
        } else {
            self.cache_eviction_manager.access(chunk_idx);
        }
        Ok(self.chunks[chunk_idx].as_mut().expect("Chunk exists"))
    }

    fn enforce_memory_limit(&mut self) -> Result<(), InternalError> {
        if let Some(max_bytes) = self.config.max_memory_bytes {
            while self.current_memory_usage > max_bytes
                && self.chunks.iter().filter(|c| c.is_some()).count() > 0
            {
                if let Some(old_idx) = self.cache_eviction_manager.evict() {
                    if let Some(chunk) = self.chunks[old_idx].take() {
                        self.current_memory_usage -= chunk.memory_usage();
                    }
                } else {
                    break;
                }
            }
        } else {
            while self.chunks.iter().filter(|c| c.is_some()).count() > self.config.max_cached_chunks
            {
                if let Some(old_idx) = self.cache_eviction_manager.evict() {
                    if let Some(chunk) = self.chunks[old_idx].take() {
                        self.current_memory_usage -= chunk.memory_usage();
                    }
                }
            }
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub fn update_item_size(&mut self, index: usize, new_size: f64) -> Result<(), JsValue> {
        if index >= self.total_items {
            return Err(convert_error(
                "IndexOutOfBounds",
                &format!("Index {} exceeds maximum {}", index, self.total_items - 1),
            ));
        }
        let chunk_idx = index / self.chunk_size;
        let item_idx = index % self.chunk_size;
        let chunk = self.get_or_create_chunk(chunk_idx).map_err(convert_internal_error)?;
        let diff = chunk.update_size(item_idx, new_size).map_err(convert_internal_error)?;
        self.total_size += diff;
        self.update_cumulative_sizes_from(chunk_idx).map_err(convert_internal_error)?;
        Ok(())
    }

    fn update_cumulative_sizes_from(&mut self, from_chunk: usize) -> Result<(), InternalError> {
        let num_chunks = self.chunks.len();
        if from_chunk >= num_chunks {
            return Err(InternalError::IndexOutOfBounds {
                index: from_chunk,
                max: num_chunks - 1,
            });
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

    #[wasm_bindgen]
    pub fn get_visible_range(
        &mut self,
        scroll_position: f64,
        viewport_size: f64,
    ) -> Result<VisibleRange, JsValue> {
        if viewport_size <= 0.0 {
            return Err(convert_error(
                "InvalidViewport",
                &format!("Viewport size must be positive, got {}", viewport_size),
            ));
        }
        if self.total_items == 0 {
            return Err(convert_error("EmptyList", "List is empty"));
        }
        let scroll_position = scroll_position.max(0.0).min(self.total_size);
        let end_position = (scroll_position + viewport_size).min(self.total_size);
        let (start_idx, start_offset) =
            self.find_item_at_position(scroll_position).map_err(convert_internal_error)?;
        let (end_idx, end_offset) =
            self.find_item_at_position(end_position).map_err(convert_internal_error)?;
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

    fn find_item_at_position(&mut self, position: f64) -> Result<(usize, f64), InternalError> {
        const MAX_SAFE_POSITION: f64 = 1e15; // Threshold for precision safety
        if self.total_items == 0 {
            return Ok((0, 0.0));
        }
        if position > MAX_SAFE_POSITION {
            return Err(InternalError::PrecisionLimitExceeded);
        }
        let (chunk_idx, position_in_chunk) =
            self.find_chunk_at_position(position).map_err(|e| e)?;
        let chunk = self.get_or_create_chunk(chunk_idx)?;
        let (item_idx, offset) = chunk.find_item_at_position(position_in_chunk)?;
        let global_idx = chunk_idx * self.chunk_size + item_idx;
        Ok((global_idx.min(self.total_items - 1), offset))
    }

    fn find_chunk_at_position(&self, position: f64) -> Result<(usize, f64), InternalError> {
        if self.total_items == 0 {
            return Err(InternalError::EmptyList);
        }
        if position.is_nan() || position < 0.0 || position > self.total_size {
            return Err(InternalError::InvalidSize { value: position });
        }
        let chunk_idx = self
            .cumulative_sizes
            .binary_search_by(|&sum| {
                sum.partial_cmp(&position).unwrap_or(std::cmp::Ordering::Greater)
            })
            .unwrap_or_else(|e| e - 1);
        let last_cumulative = if chunk_idx == 0 {
            0.0
        } else {
            self.cumulative_sizes[chunk_idx - 1]
        };
        let position_in_chunk = position - last_cumulative;
        Ok((chunk_idx, position_in_chunk))
    }

    #[wasm_bindgen]
    pub fn clear_cache(&mut self) {
        self.chunks.iter_mut().for_each(|chunk| *chunk = None);
        self.current_memory_usage = 0;
    }

    #[wasm_bindgen]
    pub fn add_items(&mut self, count: usize, size: f64) -> Result<(), JsValue> {
        if size.is_nan() || size < 0.0 {
            return Err(convert_error("InvalidSize", "Size must be non-negative"));
        }
        self.total_items += count;
        let num_chunks = (self.total_items + self.chunk_size - 1) / self.chunk_size;
        self.chunks.resize_with(num_chunks, || None);
        self.cumulative_sizes.resize(num_chunks, 0.0);
        self.total_size += size * count as f64;
        self.update_cumulative_sizes_from(0).map_err(convert_internal_error)?;
        Ok(())
    }

    #[wasm_bindgen]
    pub fn remove_items(&mut self, count: usize) -> Result<(), JsValue> {
        if count > self.total_items {
            return Err(convert_error("InvalidOperation", "Cannot remove more items than exist"));
        }
        self.total_items -= count;
        let num_chunks = (self.total_items + self.chunk_size - 1) / self.chunk_size;
        self.chunks.truncate(num_chunks);
        self.cumulative_sizes.truncate(num_chunks);
        self.total_size = self.cumulative_sizes.last().unwrap_or(&0.0).clone();
        Ok(())
    }

    #[wasm_bindgen]
    pub fn batch_update_sizes(&mut self, updates: Vec<(usize, f64)>) -> Result<(), JsValue> {
        updates.par_iter().try_for_each(|&(index, new_size)| {
            self.update_item_size(index, new_size)
        })?;
        Ok(())
    }
}