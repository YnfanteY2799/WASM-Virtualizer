use std::cmp;
use std::collections::{HashMap, VecDeque};
use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

// Enums and Structs
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub enum VirtualListError {
    IndexOutOfBounds,
    InvalidSize,
    InvalidViewport,
    InvalidConfiguration,
    EmptyList,
}

#[derive(Serialize, Deserialize)]
#[wasm_bindgen]
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

#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct VisibleRange {
    start: usize,
    end: usize,
    start_offset: f32,
    end_offset: f32,
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
    pub fn start_offset(&self) -> f32 {
        self.start_offset
    }

    #[wasm_bindgen(getter)]
    pub fn end_offset(&self) -> f32 {
        self.end_offset
    }
}

#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct VirtualListConfig {
    buffer_size: usize,
    overscan_items: usize,
    update_batch_size: usize,
    max_cached_chunks: usize,
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

    #[wasm_bindgen(getter)]
    pub fn max_cached_chunks(&self) -> usize {
        self.max_cached_chunks
    }

    #[wasm_bindgen(setter)]
    pub fn set_max_cached_chunks(&mut self, size: usize) {
        self.max_cached_chunks = size;
    }
}

// Optimized Chunk
#[derive(Clone, Debug)]
struct Chunk {
    sizes: Vec<f32>,
    prefix_sums: Vec<f32>,
    total_size: f64,
}

impl Chunk {
    fn new(chunk_size: usize, estimated_size: f32) -> Chunk {
        let estimated_size = estimated_size.max(0.0);
        let sizes = vec![estimated_size; chunk_size];
        let mut prefix_sums = Vec::with_capacity(chunk_size + 1);
        prefix_sums.push(0.0);
        let mut total_size = 0.0;

        for &size in &sizes {
            total_size += size as f64;
            prefix_sums.push(total_size as f32);
        }

        Chunk {
            sizes,
            prefix_sums,
            total_size,
        }
    }

    fn len(&self) -> usize {
        self.sizes.len()
    }

    fn update_size(&mut self, index: usize, new_size: f32) -> Result<f64, VirtualListError> {
        if index >= self.sizes.len() {
            return Err(VirtualListError::IndexOutOfBounds);
        }
        if new_size < 0.0 {
            return Err(VirtualListError::InvalidSize);
        }
        let old_size = self.sizes[index] as f64;
        let diff = new_size as f64 - old_size;
        self.sizes[index] = new_size;
        self.total_size += diff;
        for i in index + 1..self.prefix_sums.len() {
            self.prefix_sums[i] = (self.prefix_sums[i] as f64 + diff) as f32;
        }
        Ok(diff)
    }

    fn get_size(&self, index: usize) -> Result<f32, VirtualListError> {
        self.sizes.get(index).copied().ok_or(VirtualListError::IndexOutOfBounds)
    }

    fn get_position(&self, index: usize) -> Result<f32, VirtualListError> {
        self.prefix_sums.get(index).copied().ok_or(VirtualListError::IndexOutOfBounds)
    }

    fn find_item_at_position(&self, position: f32) -> Result<(usize, f32), VirtualListError> {
        if position < 0.0 || position as f64 > self.total_size {
            return Err(VirtualListError::InvalidSize);
        }
        if self.sizes.is_empty() {
            return Ok((0, 0.0));
        }
        let mut low = 0;
        let mut high = self.prefix_sums.len() - 1;

        while low < high {
            let mid = low + (high - low) / 2;
            if self.prefix_sums[mid] <= position {
                low = mid + 1;
            } else {
                high = mid;
            }
        }

        let index = if low > 0 && self.prefix_sums[low] > position {
            low - 1
        } else {
            low
        };
        let offset = position - self.prefix_sums[index];
        Ok((index, offset))
    }
}

// Optimized VirtualList
#[wasm_bindgen]
pub struct VirtualList {
    total_items: usize,
    estimated_size: f32,
    orientation: Orientation,
    chunks: HashMap<usize, Chunk>,
    chunk_size: usize,
    cumulative_sizes: HashMap<usize, f64>,
    total_size: f64,
    config: VirtualListConfig,
    pending_updates: Vec<(usize, f32)>,
    chunk_access_order: VecDeque<usize>,
}

#[wasm_bindgen]
impl VirtualList {
    #[wasm_bindgen(constructor)]
    pub fn new(total_items: usize, chunk_size: usize, estimated_size: f32, orientation: Orientation) -> VirtualList {
        let config = VirtualListConfig::new();
        Self::new_with_config(total_items, chunk_size, estimated_size, orientation, config)
    }

    pub fn new_with_config(
        total_items: usize,
        chunk_size: usize,
        estimated_size: f32,
        orientation: Orientation,
        config: VirtualListConfig,
    ) -> VirtualList {
        let chunk_size = cmp::max(1, chunk_size);
        let estimated_size = estimated_size.max(0.0);
        let total_size = estimated_size as f64 * total_items as f64;

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
            chunk_access_order: VecDeque::new(),
        }
    }

    fn get_or_create_chunk(&mut self, chunk_idx: usize) -> Result<&mut Chunk, VirtualListError> {
        if !self.chunks.contains_key(&chunk_idx) {
            let items_in_chunk = if chunk_idx == (self.total_items + self.chunk_size - 1) / self.chunk_size - 1
                && self.total_items % self.chunk_size != 0
            {
                self.total_items % self.chunk_size
            } else {
                self.chunk_size
            };
            self.chunks.insert(chunk_idx, Chunk::new(items_in_chunk, self.estimated_size));
            self.update_cumulative_sizes(chunk_idx)?;
            self.chunk_access_order.push_back(chunk_idx);

            while self.chunks.len() > self.config.max_cached_chunks() {
                if let Some(old_idx) = self.chunk_access_order.pop_front() {
                    self.chunks.remove(&old_idx);
                    self.cumulative_sizes.remove(&old_idx);
                }
            }
        } else {
            if let Some(pos) = self.chunk_access_order.iter().position(|&x| x == chunk_idx) {
                self.chunk_access_order.remove(pos);
            }
            self.chunk_access_order.push_back(chunk_idx);
        }
        Ok(self.chunks.get_mut(&chunk_idx).unwrap())
    }

    fn update_cumulative_sizes(&mut self, up_to_chunk: usize) -> Result<(), VirtualListError> {
        let num_chunks = (self.total_items + self.chunk_size - 1) / self.chunk_size;
        if up_to_chunk >= num_chunks {
            return Err(VirtualListError::IndexOutOfBounds);
        }

        let mut cumulative = 0.0;
        for i in 0..=up_to_chunk {
            cumulative += if let Some(chunk) = self.chunks.get(&i) {
                chunk.total_size
            } else {
                let items = if i == num_chunks - 1 && self.total_items % self.chunk_size != 0 {
                    self.total_items % self.chunk_size
                } else {
                    self.chunk_size
                };
                self.estimated_size as f64 * items as f64
            };
            self.cumulative_sizes.insert(i, cumulative);
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_total_size(&self) -> f64 {
        self.total_size
    }

    #[wasm_bindgen]
    pub fn get_total_items(&self) -> usize {
        self.total_items
    }

    #[wasm_bindgen]
    pub fn get_item_size(&self, index: usize) -> Result<f32, JsValue> {
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

    #[wasm_bindgen]
    pub fn update_item_size(&mut self, index: usize, new_size: f32) -> Result<(), JsValue> {
        if index >= self.total_items {
            return Err(convert_error(VirtualListError::IndexOutOfBounds));
        }
        let chunk_idx = index / self.chunk_size;
        let item_idx = index % self.chunk_size;
        let chunk = self.get_or_create_chunk(chunk_idx).map_err(convert_error)?;
        let diff = chunk.update_size(item_idx, new_size).map_err(convert_error)?;
        self.total_size += diff;
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_visible_range(&mut self, scroll_position: f32, viewport_size: f32) -> Result<VisibleRange, JsValue> {
        if viewport_size <= 0.0 {
            return Err(convert_error(VirtualListError::InvalidViewport));
        }
        if self.total_items == 0 {
            return Err(convert_error(VirtualListError::EmptyList));
        }

        let scroll_position = scroll_position.max(0.0).min(self.total_size as f32);
        let end_position = (scroll_position + viewport_size).min(self.total_size as f32);

        let (start_idx, start_offset) = self.find_item_at_position(scroll_position)?;
        let (end_idx, end_offset) = self.find_item_at_position(end_position)?;

        let buffer = self.config.buffer_size();
        let overscan = self.config.overscan_items();
        let start = start_idx.saturating_sub(buffer + overscan);
        let end = cmp::min(end_idx + buffer + overscan + 1, self.total_items);

        Ok(VisibleRange {
            start,
            end,
            start_offset,
            end_offset,
        })
    }

    fn find_chunk_at_position(&self, position: f32) -> Result<(usize, f64), VirtualListError> {
        let num_chunks = (self.total_items + self.chunk_size - 1) / self.chunk_size;
        if num_chunks == 0 {
            return Ok((0, 0.0));
        }

        let position = position as f64;
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
                mid as f64 * self.chunk_size as f64 * self.estimated_size as f64
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

    fn find_item_at_position(&mut self, position: f32) -> Result<(usize, f32), JsValue> {
        if self.total_items == 0 {
            return Ok((0, 0.0));
        }
        let position = position.max(0.0).min(self.total_size as f32);
        let (chunk_idx, position_in_chunk) = self.find_chunk_at_position(position)?;
        let chunk = self.get_or_create_chunk(chunk_idx).map_err(convert_error)?;
        let (item_idx, offset) = chunk.find_item_at_position(position_in_chunk as f32)?;
        let global_idx = chunk_idx * self.chunk_size + item_idx;
        Ok((global_idx.min(self.total_items - 1), offset))
    }

    #[wasm_bindgen]
    pub fn queue_update_item_size(&mut self, index: usize, new_size: f32) -> Result<(), JsValue> {
        if index >= self.total_items {
            return Err(convert_error(VirtualListError::IndexOutOfBounds));
        }
        if new_size < 0.0 {
            return Err(convert_error(VirtualListError::InvalidSize));
        }
        self.pending_updates.push((index, new_size));
        if self.pending_updates.len() >= self.config.update_batch_size() {
            self.process_pending_updates()?;
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub fn process_pending_updates(&mut self) -> Result<(), JsValue> {
        if self.pending_updates.is_empty() {
            return Ok(());
        }
        self.pending_updates.sort_unstable_by_key(|&(idx, _)| idx);

        let mut chunk_updates: HashMap<usize, Vec<(usize, f32)>> = HashMap::new();
        for (index, size) in self.pending_updates.drain(..) {
            let chunk_idx = index / self.chunk_size;
            let item_idx = index % self.chunk_size;
            chunk_updates.entry(chunk_idx).or_default().push((item_idx, size));
        }

        let mut total_diff = 0.0;
        for (chunk_idx, updates) in chunk_updates {
            let chunk = self.get_or_create_chunk(chunk_idx).map_err(convert_error)?;
            for (item_idx, new_size) in updates {
                total_diff += chunk.update_size(item_idx, new_size).map_err(convert_error)?;
            }
        }
        self.total_size += total_diff;
        Ok(())
    }
}