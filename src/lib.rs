use wasm_bindgen::prelude::*;

// Define the Orientation enum for the list
#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum Orientation {
    Vertical,
    Horizontal,
}

// Define the Chunk struct to hold item sizes
#[wasm_bindgen]
pub struct Chunk {
    sizes: Vec<f64>, // Sizes of items in this chunk
}

#[wasm_bindgen]
impl Chunk {
    #[wasm_bindgen(constructor)]
    pub fn new(chunk_size: usize, estimated_size: f64) -> Chunk {
        Chunk {
            sizes: vec![estimated_size; chunk_size],
        }
    }
}

// Define the VirtualList struct
#[wasm_bindgen]
pub struct VirtualList {
    total_items: usize,         // Total number of items in the list
    #[allow(dead_code)]
    estimated_size: f64,        // Default size for unmeasured items (used in JS)
    #[allow(dead_code)]
    orientation: Orientation,   // List orientation (used in JS)
    chunks: Vec<Chunk>,         // Chunks of items for efficient management
    chunk_size: usize,          // Number of items per chunk
    cumulative_sizes: Vec<f64>, // Cumulative sizes up to each chunk
}

#[wasm_bindgen]
impl VirtualList {
    /// Constructor for VirtualList
    #[wasm_bindgen(constructor)]
    pub fn new(total_items: usize, chunk_size: usize, estimated_size: f64, orientation: Orientation) -> VirtualList {
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
            running_total += chunk.sizes.iter().sum::<f64>();
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
        }
    }

    /// Get the position of an item in the list
    pub fn get_position(&self, index: usize) -> f64 {
        if index >= self.total_items {
            return 0.0; // Out of bounds, return 0 or handle differently as needed
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
        let position_in_chunk = chunk.sizes[..item_idx_in_chunk].iter().sum::<f64>();

        prev_size + position_in_chunk
    }

    /// Update the size of an item and recalculate cumulative sizes
    pub fn update_item_size(&mut self, index: usize, new_size: f64) {
        if index >= self.total_items {
            return; // Out of bounds
        }

        let chunk_idx = index / self.chunk_size;
        let item_idx_in_chunk = index % self.chunk_size;

        let chunk = &mut self.chunks[chunk_idx];
        chunk.sizes[item_idx_in_chunk] = new_size;

        // Recalculate cumulative sizes from this chunk onward
        let mut running_total = if chunk_idx > 0 {
            self.cumulative_sizes[chunk_idx - 1]
        } else {
            0.0
        };

        for i in chunk_idx..self.chunks.len() {
            running_total += self.chunks[i].sizes.iter().sum::<f64>();
            self.cumulative_sizes[i] = running_total;
        }
    }
}