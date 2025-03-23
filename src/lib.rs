use js_sys::Array;
use wasm_bindgen::prelude::*;
use serde::Serialize;
use std::cmp;
use std::collections::HashMap;

// JS error struct for error reporting
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

// Convert error to JsValue for JavaScript interop
fn convert_error(kind: &str, message: &str) -> JsValue {
    serde_wasm_bindgen::to_value(&JsError::new(kind, message)).unwrap()
}

// Orientation enum for list direction
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

// Configuration struct
#[wasm_bindgen]
#[derive(Clone)]
pub struct VirtualListConfig {
    buffer_size: usize,
    overscan_items: usize,
    #[allow(dead_code)]
    update_batch_size: usize,
}

#[wasm_bindgen]
impl VirtualListConfig {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            buffer_size: 5,
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
        self.buffer_size = size.max(1);
    }
}

// Visible range struct for rendering
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

// Chunk struct to manage item sizes with prefix sums
#[derive(Clone)]
struct Chunk {
    sizes: Vec<f64>,
    prefix_sums: Vec<f64>,
    total_size: f64,
}

impl Chunk {
    fn new(chunk_size: usize, estimated_size: f64) -> Result<Self, String> {
        if estimated_size.is_nan() || estimated_size < 0.0 {
            return Err(format!("Invalid size: {}", estimated_size));
        }
        let sizes = vec![estimated_size; chunk_size];
        let mut prefix_sums = Vec::with_capacity(chunk_size + 1);
        prefix_sums.push(0.0);
        let mut cumulative = 0.0;
        for &size in &sizes {
            cumulative += size;
            prefix_sums.push(cumulative);
        }
        let total_size = cumulative;
        Ok(Chunk {
            sizes,
            prefix_sums,
            total_size,
        })
    }

    fn update_size(&mut self, index: usize, new_size: f64) -> Result<f64, String> {
        if index >= self.sizes.len() {
            return Err(format!("Index {} out of bounds", index));
        }
        if new_size.is_nan() || new_size < 0.0 {
            return Err(format!("Invalid size: {}", new_size));
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

    fn find_item_at_position(&self, position: f64) -> Result<(usize, f64), String> {
        if position.is_nan() || position < 0.0 || position > self.total_size {
            return Err(format!("Invalid position: {}", position));
        }
        let index = self
            .prefix_sums
            .binary_search_by(|&sum| {
                sum.partial_cmp(&position).unwrap_or(cmp::Ordering::Greater)
            })
            .unwrap_or_else(|e| e - 1);
        let offset = position - self.prefix_sums[index];
        Ok((index, offset))
    }
}

// Main VirtualList struct with cumulative sizes
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
}

#[wasm_bindgen]
impl VirtualList {
    #[wasm_bindgen(constructor)]
    pub fn new(
        total_items: usize,
        chunk_size: usize,
        estimated_size: f64,
        orientation: Orientation,
        config: VirtualListConfig,
    ) -> Result<VirtualList, JsValue> {
        if chunk_size == 0 {
            return Err(convert_error("InvalidConfig", "chunk_size must be positive"));
        }
        if estimated_size.is_nan() || estimated_size < 0.0 {
            return Err(convert_error(
                "InvalidSize",
                &format!("Invalid estimated size: {}", estimated_size),
            ));
        }

        let num_chunks = (total_items + chunk_size - 1) / chunk_size;
        let mut cumulative_sizes = Vec::with_capacity(num_chunks);
        let mut total_size = 0.0;
        for i in 0..num_chunks {
            let items_in_chunk = if i == num_chunks - 1 && total_items % chunk_size != 0 {
                total_items % chunk_size
            } else {
                chunk_size
            };
            let chunk_total = estimated_size * items_in_chunk as f64;
            total_size += chunk_total;
            cumulative_sizes.push(total_size);
        }
        Ok(VirtualList {
            total_items,
            estimated_size,
            orientation,
            chunks: vec![None; num_chunks],
            chunk_size,
            cumulative_sizes,
            total_size,
            config,
        })
    }

    fn get_or_create_chunk(&mut self, chunk_idx: usize) -> Result<&mut Chunk, String> {
        if chunk_idx >= self.chunks.len() {
            return Err(format!("Chunk index {} out of bounds", chunk_idx));
        }
        if self.chunks[chunk_idx].is_none() {
            let items_in_chunk = if chunk_idx == self.chunks.len() - 1
                && self.total_items % self.chunk_size != 0
            {
                self.total_items % self.chunk_size
            } else {
                self.chunk_size
            };
            self.chunks[chunk_idx] = Some(Chunk::new(items_in_chunk, self.estimated_size)?);
        }
        Ok(self.chunks[chunk_idx].as_mut().expect("Chunk exists"))
    }

    #[wasm_bindgen]
    pub fn update_item_size(&mut self, index: usize, new_size: f64) -> Result<(), JsValue> {
        if index >= self.total_items {
            return Err(convert_error(
                "IndexOutOfBounds",
                &format!("Index {} exceeds total items", index),
            ));
        }
        let chunk_idx = index / self.chunk_size;
        let item_idx = index % self.chunk_size;
        let chunk = self
            .get_or_create_chunk(chunk_idx)
            .map_err(|e| convert_error("ChunkError", &e))?;
        let diff = chunk
            .update_size(item_idx, new_size)
            .map_err(|e| convert_error("UpdateError", &e))?;
        self.update_cumulative_sizes(chunk_idx, diff)?;
        Ok(())
    }

    fn update_cumulative_sizes(&mut self, from_chunk: usize, diff: f64) -> Result<(), String> {
        for i in from_chunk..self.cumulative_sizes.len() {
            self.cumulative_sizes[i] += diff;
        }
        self.total_size += diff;
        Ok(())
    }

    #[wasm_bindgen]
    pub fn get_visible_range(
        &mut self,
        scroll_position: f64,
        viewport_size: f64,
    ) -> Result<VisibleRange, JsValue> {
        if viewport_size <= 0.0 {
            return Err(convert_error("InvalidViewport", "Viewport size must be positive"));
        }
        if self.total_items == 0 {
            return Err(convert_error("EmptyList", "List is empty"));
        }
        let scroll_position = scroll_position.max(0.0).min(self.total_size);
        let end_position = (scroll_position + viewport_size).min(self.total_size);
        let (start_idx, start_offset) = self
            .find_item_at_position(scroll_position)
            .map_err(|e| convert_error("PositionError", &e))?;
        let (end_idx, end_offset) = self
            .find_item_at_position(end_position)
            .map_err(|e| convert_error("PositionError", &e))?;
        let buffer = self.config.buffer_size;
        let overscan = self.config.overscan_items;
        let start = start_idx.saturating_sub(buffer + overscan);
        let end = (end_idx + buffer + overscan + 1).min(self.total_items);
        Ok(VisibleRange {
            start,
            end,
            start_offset,
            end_offset,
        })
    }

    fn find_item_at_position(&mut self, position: f64) -> Result<(usize, f64), String> {
        if self.total_items == 0 {
            return Ok((0, 0.0));
        }
        let chunk_idx = self
            .cumulative_sizes
            .binary_search_by(|&sum| {
                sum.partial_cmp(&position).unwrap_or(cmp::Ordering::Greater)
            })
            .unwrap_or_else(|e| e - 1);
        let chunk_start = if chunk_idx == 0 {
            0.0
        } else {
            self.cumulative_sizes[chunk_idx - 1]
        };
        let position_in_chunk = position - chunk_start;
        let chunk = self.get_or_create_chunk(chunk_idx)?;
        let (item_idx, offset) = chunk.find_item_at_position(position_in_chunk)?;
        let global_idx = chunk_idx * self.chunk_size + item_idx;
        Ok((global_idx.min(self.total_items - 1), offset))
    }

    #[wasm_bindgen]
    pub fn batch_update_sizes(&mut self, updates: Vec<JsValue>) -> Result<(), JsValue> {
        let parsed_updates: Vec<Result<(usize, f64), String>> = updates
            .into_iter()
            .map(|js_val| {
                let arr = js_val
                    .dyn_into::<Array>()
                    .map_err(|_| "Invalid update format".to_string())?;
                if arr.length() != 2 {
                    return Err("Each update must be an array of [index, size]".to_string());
                }
                let index = arr
                    .get(0)
                    .as_f64()
                    .ok_or("Index must be a number".to_string())? as usize;
                let size = arr
                    .get(1)
                    .as_f64()
                    .ok_or("Size must be a number".to_string())?;
                Ok((index, size))
            })
            .collect();

        let updates: Vec<(usize, f64)> = parsed_updates
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| convert_error("InvalidUpdate", &e))?;

        // Group updates by chunk index
        let mut chunk_updates: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
        for (index, new_size) in updates {
            if index >= self.total_items {
                return Err(convert_error(
                    "IndexOutOfBounds",
                    &format!("Index {} out of bounds", index),
                ));
            }
            let chunk_idx = index / self.chunk_size;
            let item_idx = index % self.chunk_size;
            chunk_updates
                .entry(chunk_idx)
                .or_insert_with(Vec::new)
                .push((item_idx, new_size));
        }

        // Apply updates and calculate net difference per chunk
        let mut chunk_diffs: HashMap<usize, f64> = HashMap::new();
        for (chunk_idx, updates) in chunk_updates {
            let chunk = self
                .get_or_create_chunk(chunk_idx)
                .map_err(|e| convert_error("ChunkError", &e))?;
            let mut total_diff = 0.0;
            for (item_idx, new_size) in updates {
                let diff = chunk
                    .update_size(item_idx, new_size)
                    .map_err(|e| convert_error("UpdateError", &e))?;
                total_diff += diff;
            }
            chunk_diffs.insert(chunk_idx, total_diff);
        }

        // Update cumulative sizes in a single pass
        let min_chunk_idx = chunk_diffs.keys().min().cloned().unwrap_or(0);
        let mut cumulative_diff = 0.0;
        for i in min_chunk_idx..self.chunks.len() {
            if let Some(diff) = chunk_diffs.get(&i) {
                cumulative_diff += diff;
            }
            if i < self.cumulative_sizes.len() {
                self.cumulative_sizes[i] += cumulative_diff;
            }
        }
        self.total_size += cumulative_diff;

        Ok(())
    }
}