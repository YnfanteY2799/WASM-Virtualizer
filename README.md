# 🚀 Rust WASM Virtual Scrolling Library

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-performance virtual scrolling implementation written in Rust and compiled to WebAssembly. This library efficiently handles large lists with variable-sized items by only rendering what's visible in the viewport.

## ✨ Features

- 📜 Handle virtually infinite lists with minimal memory usage
- 📏 Support for variable-sized items
- 🧩 Chunking strategy for efficient memory management
- 🔄 Dynamic updates to item sizes
- 📊 Precise scroll position calculations
- 🔌 WebAssembly integration with JavaScript
- 🔀 Support for both horizontal and vertical scrolling
- 🧠 Smart memory management with LRU chunk unloading

## 📋 Use Cases

- **Large Data Sets**: Render tables with thousands or millions of rows efficiently
- **Social Media Feeds**: Implement infinite scrolling timelines with variable-sized content
- **Virtual Kanban Boards**: Display boards with dynamically sized cards
- **Chat Applications**: Show message history with differently sized messages
- **File Explorers**: List thousands of files with varying row heights based on content
- **Image Galleries**: Display collections with differently sized thumbnails

## 🛠️ Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (1.58.0 or later)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- [Node.js](https://nodejs.org/) (for npm)

### Building the Library

1. Clone the repository:

```bash
git clone https://github.com/your-username/rust-wasm-virtual-scroll.git
cd rust-wasm-virtual-scroll
```

2. Build the WebAssembly package:

```bash
wasm-pack build --target web
```

This will generate a `pkg` directory with the compiled WebAssembly module and JavaScript bindings.

## 🔧 Usage

### Basic Integration

```javascript
import { VirtualList, VirtualListConfig, Orientation } from "rust-wasm-virtual-scroll";

// Create a configuration
const config = new VirtualListConfig();
config.buffer_size = 5; // Number of items to render outside viewport
config.overscan_items = 3; // Additional items to render
config.max_loaded_chunks = 100; // Maximum chunks to keep in memory

// Initialize the virtual list
const totalItems = 10000; // Total number of items
const chunkSize = 100; // Items per chunk
const estimatedItemSize = 50; // Initial estimated height (px)
const orientation = Orientation.Vertical; // Scrolling direction

const virtualList = new VirtualList(totalItems, chunkSize, estimatedItemSize, orientation, config);

// Get visible range based on scroll position
function updateVisibleItems(scrollTop, viewportHeight) {
 try {
  const visibleRange = virtualList.get_visible_range(scrollTop, viewportHeight);

  // Render only items in the visible range
  for (let i = visibleRange.start; i < visibleRange.end; i++) {
   // Render item at index i
   const itemElement = renderItem(i);

   // After rendering, measure and update actual size
   const actualSize = itemElement.offsetHeight;
   virtualList.update_item_size(i, actualSize);
  }
 } catch (e) {
  console.error("Error updating visible items:", e);
 }
}

// Listen to scroll events
document.getElementById("scroll-container").addEventListener("scroll", (e) => {
 const scrollTop = e.target.scrollTop;
 const viewportHeight = e.target.clientHeight;
 updateVisibleItems(scrollTop, viewportHeight);
});
```

### Batch Updates

```javascript
// Update multiple item sizes at once for better performance
const updates = [
 [0, 75], // [index, size]
 [1, 120],
 [2, 60],
];
virtualList.batch_update_sizes(updates);
```

### Dynamically Changing List Size

```javascript
// When the total number of items changes:
virtualList.set_total_items(newTotalItems);
```

### Manual Chunk Management

```javascript
// Manually unload a chunk to free memory
virtualList.unload_chunk(chunkIndex);
```

### React Integration example

```jsx
// React Integration Example for Rust WASM Virtual Scrolling Library
import React, { useEffect, useRef, useState, useCallback } from 'react';
import { VirtualList, VirtualListConfig, Orientation } from 'rust-wasm-virtual-scroll';

const VirtualScrollList = ({ items, itemHeight = 50, totalItems = 10000 }) => {
  const [visibleItems, setVisibleItems] = useState([]);
  const [totalHeight, setTotalHeight] = useState(0);
  const containerRef = useRef(null);
  const virtualListRef = useRef(null);
  const itemsRef = useRef({});

  // Initialize virtual list
  useEffect(() => {
    const config = new VirtualListConfig();
    config.buffer_size = 5;
    config.overscan_items = 3;
    config.max_loaded_chunks = 100;

    const virtualList = new VirtualList(
      totalItems,
      100, // chunk size
      itemHeight,
      Orientation.Vertical,
      config
    );

    virtualListRef.current = virtualList;
    setTotalHeight(virtualList.total_size);

    return () => {
      // Clean up when component unmounts
      virtualListRef.current = null;
    };
  }, [totalItems, itemHeight]);

  // Handle scroll events
  const handleScroll = useCallback(() => {
    if (!containerRef.current || !virtualListRef.current) return;

    const { scrollTop, clientHeight } = containerRef.current;
    try {
      const visibleRange = virtualListRef.current.get_visible_range(scrollTop, clientHeight);
      
      // Create array of visible items with their positions
      const newVisibleItems = [];
      for (let i = visibleRange.start; i < visibleRange.end; i++) {
        if (i < items.length) {
          newVisibleItems.push({
            index: i,
            data: items[i],
            // Calculate position - this is simplified
            top: i === visibleRange.start 
              ? scrollTop - visibleRange.start_offset 
              : undefined
          });
        }
      }
      setVisibleItems(newVisibleItems);
    } catch (error) {
      console.error("Error in virtual scrolling:", error);
    }
  }, [items]);

  // Update item heights after render
  useEffect(() => {
    if (!virtualListRef.current) return;
    
    // Batch updates for better performance
    const updates = [];
    
    visibleItems.forEach(item => {
      const element = itemsRef.current[item.index];
      if (element) {
        const height = element.offsetHeight;
        updates.push([item.index, height]);
      }
    });
    
    if (updates.length > 0) {
      virtualListRef.current.batch_update_sizes(updates);
      setTotalHeight(virtualListRef.current.total_size);
    }
  }, [visibleItems]);

  // Set up scroll listener
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    
    container.addEventListener('scroll', handleScroll);
    handleScroll(); // Initial calculation
    
    return () => {
      container.removeEventListener('scroll', handleScroll);
    };
  }, [handleScroll]);

  return (
    <div 
      ref={containerRef}
      style={{ 
        height: '500px', 
        overflow: 'auto',
        position: 'relative',
        border: '1px solid #ccc',
        borderRadius: '4px'
      }}
    >
      <div style={{ height: `${totalHeight}px`, position: 'relative' }}>
        {visibleItems.map(item => (
          <div
            key={item.index}
            ref={el => itemsRef.current[item.index] = el}
            style={{
              position: 'absolute',
              top: item.top !== undefined ? `${item.top}px` : undefined,
              left: 0,
              width: '100%'
            }}
          >
            {/* Render your item content here */}
            <div style={{ padding: '8px', borderBottom: '1px solid #eee' }}>
              Item {item.index}: {JSON.stringify(item.data)}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

export default VirtualScrollList;

// Usage example
function App() {
  // Generate sample data
  const sampleItems = Array.from({ length: 10000 }, (_, i) => ({
    id: i,
    text: `Item ${i}`,
    description: `This is a description for item ${i}`
  }));

  return (
    <div className="App">
      <h1>Virtual Scroll with Rust WASM</h1>
      <VirtualScrollList 
        items={sampleItems}
        itemHeight={50}
        totalItems={sampleItems.length}
      />
    </div>
  );
}
```

### Preact Integration Example

```jsx
// Preact Integration Example for Rust WASM Virtual Scrolling Library
/** @jsx h */
import { h, Component } from 'preact';
import { useEffect, useRef, useState } from 'preact/hooks';
import { VirtualList, VirtualListConfig, Orientation } from 'rust-wasm-virtual-scroll';

// Functional component implementation with hooks
const VirtualScrollList = ({ items, itemHeight = 50, totalItems = 10000 }) => {
  const [visibleItems, setVisibleItems] = useState([]);
  const [totalHeight, setTotalHeight] = useState(0);
  const containerRef = useRef(null);
  const virtualListRef = useRef(null);
  const itemsRef = useRef({});
  const resizeObserverRef = useRef(null);

  // Initialize virtual list
  useEffect(() => {
    const config = new VirtualListConfig();
    config.buffer_size = 5;
    config.overscan_items = 5; // Slightly more overscan for Preact for smoother scrolling
    config.max_loaded_chunks = 80;

    const virtualList = new VirtualList(
      totalItems,
      100, // chunk size
      itemHeight,
      Orientation.Vertical,
      config
    );

    virtualListRef.current = virtualList;
    setTotalHeight(virtualList.total_size);

    // Initialize ResizeObserver to handle container resizing
    resizeObserverRef.current = new ResizeObserver(() => {
      if (containerRef.current) {
        updateVisibleItems();
      }
    });

    if (containerRef.current) {
      resizeObserverRef.current.observe(containerRef.current);
    }

    return () => {
      // Clean up when component unmounts
      if (resizeObserverRef.current) {
        resizeObserverRef.current.disconnect();
      }
      virtualListRef.current = null;
    };
  }, [totalItems, itemHeight]);

  // Update visible items based on scroll position
  const updateVisibleItems = () => {
    if (!containerRef.current || !virtualListRef.current) return;

    const { scrollTop, clientHeight } = containerRef.current;
    try {
      const visibleRange = virtualListRef.current.get_visible_range(scrollTop, clientHeight);
      
      // Create array of visible items with their positions
      const itemsToRender = [];
      let currentOffset = 0;
      
      for (let i = visibleRange.start; i < visibleRange.end; i++) {
        if (i < items.length) {
          // For first item, use the offset from visible range
          if (i === visibleRange.start) {
            currentOffset = scrollTop - visibleRange.start_offset;
          }
          
          itemsToRender.push({
            index: i,
            data: items[i],
            top: currentOffset
          });
          
          // For subsequent items, we need to calculate based on previous items
          // This is simplified - ideally would use actual sizes from virtualList
          if (itemsRef.current[i]) {
            currentOffset += itemsRef.current[i].offsetHeight;
          } else {
            currentOffset += itemHeight;
          }
        }
      }
      
      setVisibleItems(itemsToRender);
    } catch (error) {
      console.error("Error in virtual scrolling:", error);
    }
  };

  // Update item heights after render
  useEffect(() => {
    if (!virtualListRef.current) return;
    
    const updates = [];
    let needsUpdate = false;
    
    visibleItems.forEach(item => {
      const element = itemsRef.current[item.index];
      if (element) {
        const height = element.offsetHeight;
        // Only update if size changed
        if (height !== item.height) {
          updates.push([item.index, height]);
          needsUpdate = true;
        }
      }
    });
    
    if (needsUpdate && updates.length > 0) {
      virtualListRef.current.batch_update_sizes(updates);
      setTotalHeight(virtualListRef.current.total_size);
      // After updating sizes, we should recalculate visible items
      // Using setTimeout to avoid render loops
      setTimeout(updateVisibleItems, 0);
    }
  }, [visibleItems]);

  // Set up scroll listener
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    
    const handleScroll = () => {
      window.requestAnimationFrame(updateVisibleItems);
    };
    
    container.addEventListener('scroll', handleScroll);
    updateVisibleItems(); // Initial calculation
    
    return () => {
      container.removeEventListener('scroll', handleScroll);
    };
  }, [items]);

  return (
    <div 
      ref={containerRef}
      style={{ 
        height: '500px', 
        overflow: 'auto',
        position: 'relative',
        border: '1px solid #ccc',
        borderRadius: '4px'
      }}
    >
      <div style={{ height: `${totalHeight}px`, position: 'relative' }}>
        {visibleItems.map(item => (
          <div
            key={item.index}
            ref={el => itemsRef.current[item.index] = el}
            style={{
              position: 'absolute',
              top: `${item.top}px`,
              left: 0,
              width: '100%'
            }}
          >
            <div style={{ padding: '8px', borderBottom: '1px solid #eee' }}>
              Item {item.index}: {JSON.stringify(item.data)}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
};

// Class component alternative implementation
class VirtualScrollListClass extends Component {
  constructor(props) {
    super(props);
    this.state = {
      visibleItems: [],
      totalHeight: 0
    };
    this.containerRef = null;
    this.virtualList = null;
    this.itemRefs = {};
  }

  componentDidMount() {
    const { itemHeight = 50, totalItems = 10000 } = this.props;
    
    const config = new VirtualListConfig();
    config.buffer_size = 5;
    config.overscan_items = 5;
    config.max_loaded_chunks = 80;

    this.virtualList = new VirtualList(
      totalItems,
      100,
      itemHeight,
      Orientation.Vertical,
      config
    );

    this.setState({ totalHeight: this.virtualList.total_size });
    
    if (this.containerRef) {
      this.containerRef.addEventListener('scroll', this.handleScroll);
      this.updateVisibleItems(); // Initial calculation
    }
  }

  componentWillUnmount() {
    if (this.containerRef) {
      this.containerRef.removeEventListener('scroll', this.handleScroll);
    }
  }

  handleScroll = () => {
    window.requestAnimationFrame(this.updateVisibleItems);
  };

  updateVisibleItems = () => {
    // Implementation similar to the functional component
    if (!this.containerRef || !this.virtualList) return;
    
    // Similar implementation to functional component...
  };

  setContainerRef = (ref) => {
    this.containerRef = ref;
  };

  setItemRef = (index, ref) => {
    this.itemRefs[index] = ref;
  };

  render() {
    const { visibleItems, totalHeight } = this.state;
    
    return (
      <div 
        ref={this.setContainerRef}
        style={{ 
          height: '500px', 
          overflow: 'auto',
          position: 'relative',
          border: '1px solid #ccc'
        }}
      >
        <div style={{ height: `${totalHeight}px`, position: 'relative' }}>
          {visibleItems.map(item => (
            <div
              key={item.index}
              ref={(ref) => this.setItemRef(item.index, ref)}
              style={{
                position: 'absolute',
                top: `${item.top}px`,
                left: 0,
                width: '100%'
              }}
            >
              <div style={{ padding: '8px', borderBottom: '1px solid #eee' }}>
                Item {item.index}: {JSON.stringify(item.data)}
              </div>
            </div>
          ))}
        </div>
      </div>
    );
  }
}

export { VirtualScrollList, VirtualScrollListClass };

// Usage example
export default function App() {
  const sampleItems = Array.from({ length: 10000 }, (_, i) => ({
    id: i,
    text: `Item ${i}`,
    complex: i % 3 === 0 ? "This is a taller item with more content" : "Standard"
  }));

  return (
    <div>
      <h1>Preact Virtual Scroll with Rust WASM</h1>
      <VirtualScrollList 
        items={sampleItems}
        itemHeight={50}
        totalItems={sampleItems.length}
      />
    </div>
  );
}
```

### VueJs Integration Example

```vue
// Vue.js Integration Example for Rust WASM Virtual Scrolling Library

// VirtualScrollList.vue
<template>
  <div 
    ref="container" 
    class="virtual-scroll-container"
    @scroll="handleScroll"
  >
    <div 
      class="virtual-scroll-content" 
      :style="{ height: `${totalHeight}px` }"
    >
      <div
        v-for="item in visibleItems"
        :key="item.index"
        :ref="el => setItemRef(item.index, el)"
        class="virtual-scroll-item"
        :style="{ transform: `translateY(${item.top}px)` }"
      >
        <slot name="item" :item="item.data" :index="item.index">
          <!-- Default content if no slot is provided -->
          <div class="item-content">
            Item {{ item.index }}: {{ JSON.stringify(item.data) }}
          </div>
        </slot>
      </div>
    </div>
  </div>
</template>

<script>
import { nextTick, onMounted, onUnmounted, ref, reactive, watch, computed } from 'vue';
import { VirtualList, VirtualListConfig, Orientation } from 'rust-wasm-virtual-scroll';

export default {
  name: 'VirtualScrollList',
  
  props: {
    items: {
      type: Array,
      required: true
    },
    itemHeight: {
      type: Number,
      default: 50
    },
    totalItems: {
      type: Number,
      default() {
        return this.items.length;
      }
    },
    orientation: {
      type: String,
      default: 'vertical',
      validator: value => ['vertical', 'horizontal'].includes(value)
    },
    bufferSize: {
      type: Number,
      default: 5
    },
    overscanItems: {
      type: Number,
      default: 3
    },
    maxLoadedChunks: {
      type: Number,
      default: 100
    }
  },
  
  setup(props) {
    const container = ref(null);
    const virtualList = ref(null);
    const itemRefs = reactive({});
    const visibleItems = ref([]);
    const totalHeight = ref(0);
    let resizeObserver = null;
    let scrollThrottleId = null;

    // Initialize virtual list
    const initVirtualList = () => {
      const config = new VirtualListConfig();
      config.buffer_size = props.bufferSize;
      config.overscan_items = props.overscanItems;
      config.max_loaded_chunks = props.maxLoadedChunks;

      virtualList.value = new VirtualList(
        props.totalItems,
        100, // chunk size
        props.itemHeight,
        props.orientation === 'vertical' ? Orientation.Vertical : Orientation.Horizontal,
        config
      );

      totalHeight.value = virtualList.value.total_size;
    };

    // Handle scroll events with throttling
    const handleScroll = () => {
      if (scrollThrottleId) {
        cancelAnimationFrame(scrollThrottleId);
      }
      
      scrollThrottleId = requestAnimationFrame(() => {
        updateVisibleItems();
      });
    };

    // Update visible items based on scroll position
    const updateVisibleItems = () => {
      if (!container.value || !virtualList.value) return;

      const { scrollTop, clientHeight } = container.value;
      
      try {
        const range = virtualList.value.get_visible_range(scrollTop, clientHeight);
        
        // Calculate positions for visible items
        const items = [];
        
        for (let i = range.start; i < range.end; i++) {
          if (i < props.items.length) {
            // Calculate position for this item
            let top;
            
            if (i === range.start) {
              top = scrollTop - range.start_offset;
            } else {
              // For other items, we need to calculate the position
              // For simplicity, we're using the virtual list's calculations
              // This is a simplified approach
              const prevPosition = virtualList.value.find_item_position(i - 1);
              const currentPosition = virtualList.value.find_item_position(i);
              top = currentPosition - (scrollTop - prevPosition);
            }
            
            items.push({
              index: i,
              data: props.items[i],
              top: top
            });
          }
        }
        
        visibleItems.value = items;
      } catch (error) {
        console.error("Error updating visible items:", error);
      }
    };

    // Update item heights after they're rendered
    const updateItemSizes = async () => {
      if (!virtualList.value) return;
      
      const updates = [];
      let needsUpdate = false;
      
      for (const item of visibleItems.value) {
        const element = itemRefs[item.index];
        if (element) {
          const height = element.offsetHeight;
          updates.push([item.index, height]);
          needsUpdate = true;
        }
      }
      
      if (needsUpdate && updates.length > 0) {
        virtualList.value.batch_update_sizes(updates);
        totalHeight.value = virtualList.value.total_size;
        
        // Re-calculate visible items after sizes update
        await nextTick();
        updateVisibleItems();
      }
    };

    // Set item ref for measuring
    const setItemRef = (index, el) => {
      if (el) {
        itemRefs[index] = el;
      }
    };

    // Watch for prop changes
    watch(() => props.totalItems, (newValue) => {
      if (virtualList.value && newValue !== virtualList.value.total_items) {
        virtualList.value.set_total_items(newValue);
        totalHeight.value = virtualList.value.total_size;
        updateVisibleItems();
      }
    });

    // Set up when component is mounted
    onMounted(() => {
      initVirtualList();
      
      // Set up resize observer to handle container size changes
      resizeObserver = new ResizeObserver(() => {
        updateVisibleItems();
      });
      
      if (container.value) {
        resizeObserver.observe(container.value);
      }
      
      updateVisibleItems();
      
      // Observe for changes in item heights
      watch(visibleItems, () => {
        nextTick(() => {
          updateItemSizes();
        });
      });
    });

    // Clean up when component is unmounted
    onUnmounted(() => {
      if (resizeObserver) {
        resizeObserver.disconnect();
      }
      
      if (scrollThrottleId) {
        cancelAnimationFrame(scrollThrottleId);
      }
      
      virtualList.value = null;
    });

    return {
      container,
      visibleItems,
      totalHeight,
      handleScroll,
      setItemRef
    };
  }
};
</script>

<style scoped>
.virtual-scroll-container {
  height: 500px;
  overflow: auto;
  position: relative;
  border: 1px solid #ccc;
  border-radius: 4px;
}

.virtual-scroll-content {
  position: relative;
}

.virtual-scroll-item {
  position: absolute;
  left: 0;
  width: 100%;
}

.item-content {
  padding: 8px;
  border-bottom: 1px solid #eee;
}
</style>

// Example usage in App.vue
<template>
  <div class="app">
    <h1>Vue.js Virtual Scroll with Rust WASM</h1>
    <virtual-scroll-list :items="items" :item-height="60">
      <template #item="{ item, index }">
        <div class="custom-item" :class="{ 'even': index % 2 === 0 }">
          <div class="item-title">Item {{ index }}</div>
          <div class="item-details">{{ item.text }}</div>
          <div v-if="item.details" class="item-extra">{{ item.details }}</div>
        </div>
      </template>
    </virtual-scroll-list>
  </div>
</template>

<script>
import VirtualScrollList from './components/VirtualScrollList.vue';

export default {
  name: 'App',
  components: {
    VirtualScrollList
  },
  setup() {
    // Generate sample items with variable content
    const items = Array.from({ length: 10000 }, (_, i) => ({
      id: i,
      text: `This is item ${i}`,
      details: i % 3 === 0 ? `Additional details for item ${i}` : null
    }));

    return {
      items
    };
  }
};
</script>

<style>
.app {
  font-family: Avenir, Helvetica, Arial, sans-serif;
  max-width: 800px;
  margin: 0 auto;
  padding: 20px;
}

.custom-item {
  padding: 10px;
  border-bottom: 1px solid #eee;
}

.even {
  background-color: #f9f9f9;
}

.item-title {
  font-weight: bold;
}

.item-details {
  color: #666;
}

.item-extra {
  margin-top: 8px;
  font-style: italic;
  color: #999;
}
</style>
```

## 🏗️ Architecture

The library is built around these key components:

1. **VirtualList**: The main class that manages the entire virtual list. It handles:

   - Tracking total items
   - Calculating visible ranges
   - Managing chunks
   - Updating item sizes

2. **Chunk**: Manages a subset of items in the list:

   - Stores actual sizes for items
   - Maintains prefix sums for fast position calculations
   - Handles updates to item sizes

3. **VirtualListConfig**: Configuration options for the virtual list:

   - `buffer_size`: Additional items to render before and after viewport
   - `overscan_items`: Extra items to render for smoother scrolling
   - `max_loaded_chunks`: Memory management parameter

4. **Memory Management**: Uses a Least Recently Used (LRU) strategy to unload chunks when memory limits are reached.

## ⚠️ Error Handling

The library provides detailed error information for common issues:

- Index out of bounds
- Invalid size values
- Chunk creation failures
- Position calculation errors

All errors are converted to a standard JavaScript Error format with `kind` and `message` properties.

## 🔄 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📝 License

This project is licensed under the MIT License - see the LICENSE file for details.
