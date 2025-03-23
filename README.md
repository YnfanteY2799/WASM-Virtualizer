# 🚀 Rust WASM Virtual List

A high-performance virtual list implementation in Rust, compiled to WebAssembly for seamless integration with JavaScript applications.

## ✨ Features

- **Virtualized Rendering**: Only render what's visible in the viewport
- **Variable Heights**: Support for dynamic item sizes
- **Memory Efficient**: Chunk-based data storage with LRU caching
- **Smooth Scrolling**: Precise calculations for scroll positions
- **WebAssembly Speed**: Leverage Rust's performance in the browser
- **Bidirectional Orientation**: Support for both vertical and horizontal lists

## 📋 Use Cases

- **Infinite Lists**: Handle millions of items with minimal memory usage
- **Dynamic Content**: Update item sizes on the fly as content changes
- **Social Media Feeds**: Efficiently render feeds with variable content sizes
- **Data Tables**: Display large datasets with optimized performance
- **Chat Applications**: Render message history with varying message sizes
- **Product Catalogs**: Display large inventories with different product card heights

## 🛠️ Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- [Node.js](https://nodejs.org/) (for JavaScript integration)

### Compilation Steps

1. **Clone the repository**

```bash
git clone https://github.com/yourusername/rust-wasm-virtual-list.git
cd rust-wasm-virtual-list
```

2. **Build the WebAssembly package**

```bash
wasm-pack build --target web
```

3. **Include in your web project**

```bash
# If using npm
npm install --save ./pkg

# If using yarn
yarn add ./pkg
```

## 📚 Usage

### Basic Example

```javascript
import { VirtualList, VirtualListConfig, Orientation } from "rust-wasm-virtual-list";

// Create configuration
const config = new VirtualListConfig();
config.set_buffer_size(5);
config.set_overscan_items(3);
config.set_max_loaded_chunks(100);

// Initialize the virtual list
const totalItems = 10000;
const chunkSize = 100;
const estimatedItemHeight = 30;
const list = new VirtualList(totalItems, chunkSize, estimatedItemHeight, Orientation.Vertical, config);

// Get visible range on scroll
function handleScroll(scrollTop, viewportHeight) {
 const visibleRange = list.get_visible_range(scrollTop, viewportHeight);

 // Render only items from visibleRange.start to visibleRange.end
 renderItems(visibleRange.start, visibleRange.end);
}

// Update item size after rendering or resizing
function updateItemSize(index, actualSize) {
 list.update_item_size(index, actualSize);
}

// Batch update multiple items at once for better performance
function batchUpdate(updates) {
 // updates format: [[index1, size1], [index2, size2], ...]
 list.batch_update_sizes(updates);
}
```

### React Integration Example

```jsx
import React, { useRef, useEffect, useState } from "react";
import { VirtualList, VirtualListConfig, Orientation } from "rust-wasm-virtual-list";

function VirtualizedList({ items, itemRenderer, estimatedHeight }) {
 const [virtualList, setVirtualList] = useState(null);
 const [visibleItems, setVisibleItems] = useState([]);
 const containerRef = useRef(null);

 // Initialize virtual list
 useEffect(() => {
  const config = new VirtualListConfig();
  const list = new VirtualList(items.length, 100, estimatedHeight, Orientation.Vertical, config);
  setVirtualList(list);

  return () => {
   list.free(); // Clean up WASM resources
  };
 }, [items.length, estimatedHeight]);

 // Handle scroll events
 useEffect(() => {
  if (!virtualList || !containerRef.current) return;

  const handleScroll = () => {
   const { scrollTop, clientHeight } = containerRef.current;
   const range = virtualList.get_visible_range(scrollTop, clientHeight);

   setVisibleItems(
    items.slice(range.start, range.end).map((item, i) => ({
     item,
     index: range.start + i,
     offsetTop: i === 0 ? range.start_offset : undefined,
    }))
   );
  };

  containerRef.current.addEventListener("scroll", handleScroll);
  handleScroll(); // Initial render

  return () => {
   containerRef.current?.removeEventListener("scroll", handleScroll);
  };
 }, [virtualList, items]);

 // Update item sizes after render
 useEffect(() => {
  if (!virtualList) return;

  const itemElements = document.querySelectorAll(".virtual-item");
  const updates = [];

  itemElements.forEach((el) => {
   const index = parseInt(el.dataset.index, 10);
   const height = el.offsetHeight;
   updates.push([index, height]);
  });

  if (updates.length > 0) {
   virtualList.batch_update_sizes(updates);
  }
 }, [visibleItems, virtualList]);

 return (
  <div ref={containerRef} style={{ height: "500px", overflow: "auto" }}>
   <div style={{ height: `${virtualList?.total_size || 0}px`, position: "relative" }}>
    {visibleItems.map(({ item, index, offsetTop }) => (
     <div
      key={index}
      className="virtual-item"
      data-index={index}
      style={{
       position: "absolute",
       top: offsetTop !== undefined ? offsetTop : undefined,
       width: "100%",
      }}>
      {itemRenderer(item, index)}
     </div>
    ))}
   </div>
  </div>
 );
}
```

## 🔍 API Reference

### VirtualListConfig

Configuration options for the virtual list behavior.

| Method                                               | Description                                         |
| ---------------------------------------------------- | --------------------------------------------------- |
| `new()`                                              | Create a new config with default settings           |
| `buffer_size()` / `set_buffer_size(size)`            | Number of items to buffer before/after visible area |
| `overscan_items()` / `set_overscan_items(items)`     | Additional items to render beyond buffer            |
| `max_loaded_chunks()` / `set_max_loaded_chunks(max)` | Maximum number of chunks to keep in memory          |

### VirtualList

Core virtual list implementation.

| Method                                                           | Description                                      |
| ---------------------------------------------------------------- | ------------------------------------------------ |
| `new(totalItems, chunkSize, estimatedSize, orientation, config)` | Create a new virtual list                        |
| `update_item_size(index, newSize)`                               | Update the size of a specific item               |
| `get_visible_range(scrollPosition, viewportSize)`                | Calculate visible range based on scroll position |
| `batch_update_sizes(updates)`                                    | Update multiple item sizes at once               |
| `set_total_items(newTotal)`                                      | Change the total number of items in the list     |
| `unload_chunk(chunkIdx)`                                         | Manually unload a specific chunk from memory     |

### VisibleRange

Result of the `get_visible_range` method.

| Property       | Description                            |
| -------------- | -------------------------------------- |
| `start`        | Index of the first visible item        |
| `end`          | Index of the last visible item         |
| `start_offset` | Pixel offset of the first visible item |
| `end_offset`   | Pixel offset of the last visible item  |

## ⚙️ Performance Tuning

- **Chunk Size**: Smaller chunks use less memory but may require more frequent loads. Larger chunks reduce overhead but use more memory.
- **Buffer Size**: Increase for smoother scrolling at the cost of more rendering work.
- **Overscan Items**: More items rendered beyond the viewport helps prevent empty space during fast scrolling.
- **Max Loaded Chunks**: Adjust based on your application's memory constraints.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.
