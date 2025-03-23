#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    // Initialize wasm test environment
    wasm_bindgen_test_configure!(run_in_browser);

    #[test]
    fn test_empty_list() {
        let list = VirtualList::new(0, 10.0, Orientation::Vertical, 10);
        let visible = list.compute_visible_range(0.0, 100.0, 0);
        assert_eq!(
            visible.len(),
            0,
            "Empty list should return empty visible items"
        );
    }

    #[test]
    fn test_zero_viewport() {
        let list = VirtualList::new(100, 10.0, Orientation::Vertical, 10);
        let visible = list.compute_visible_range(0.0, 0.0, 0);
        assert_eq!(
            visible.len(),
            0,
            "Zero viewport should return empty visible items"
        );
    }

    #[test]
    fn test_negative_scroll() {
        let list = VirtualList::new(100, 10.0, Orientation::Vertical, 10);
        let visible = list.compute_visible_range(-50.0, 100.0, 0);
        assert!(
            visible.len() > 0,
            "Negative scroll should be handled gracefully"
        );
        assert_eq!(
            visible[0].index(),
            0,
            "First visible item should be at index 0"
        );
    }

    #[test]
    fn test_scroll_beyond_end() {
        let list = VirtualList::new(100, 10.0, Orientation::Vertical, 10);
        // Scroll beyond the end of the list (100 items * 10.0 size = 1000.0 total size)
        let visible = list.compute_visible_range(1500.0, 100.0, 0);
        assert_eq!(
            visible.len(),
            0,
            "Scrolling beyond the end should return empty visible items"
        );
    }

    #[test]
    fn test_overscan() {
        let list = VirtualList::new(100, 10.0, Orientation::Vertical, 10);
        // View port shows items 10-19 (scroll position 100.0, viewport size 100.0)
        let no_overscan = list.compute_visible_range(100.0, 100.0, 0);
        let with_overscan = list.compute_visible_range(100.0, 100.0, 2);

        assert!(
            with_overscan.len() > no_overscan.len(),
            "Overscan should increase visible items"
        );
        assert!(
            with_overscan[0].index() < no_overscan[0].index(),
            "Overscan should include items before visible range"
        );
        assert!(
            with_overscan[with_overscan.len() - 1].index()
                > no_overscan[no_overscan.len() - 1].index(),
            "Overscan should include items after visible range"
        );
    }

    #[test]
    fn test_chunk_boundary() {
        // Create list with 3 items per chunk
        let list = VirtualList::new(10, 10.0, Orientation::Vertical, 3);
        // Check that items at chunk boundaries are positioned correctly
        assert_eq!(list.get_position(2), 20.0, "Last item in first chunk");
        assert_eq!(list.get_position(3), 30.0, "First item in second chunk");
    }

    #[test]
    fn test_variable_sizes() {
        let mut list = VirtualList::new(5, 10.0, Orientation::Vertical, 2);

        // Update item sizes
        list.update_item_sizes(&[0, 2, 4], &[20.0, 30.0, 15.0])
            .unwrap();

        // Check positions
        assert_eq!(list.get_position(0), 0.0, "First item starts at 0");
        assert_eq!(
            list.get_position(1),
            20.0,
            "Second item starts after first item (size 20)"
        );
        assert_eq!(
            list.get_position(2),
            30.0,
            "Third item starts after second item (size 10)"
        );
        assert_eq!(
            list.get_position(3),
            60.0,
            "Fourth item starts after third item (size 30)"
        );
        assert_eq!(
            list.get_position(4),
            70.0,
            "Fifth item starts after fourth item (size 10)"
        );

        // Check visible range
        let visible = list.compute_visible_range(25.0, 20.0, 0);
        assert_eq!(
            visible.len(),
            2,
            "Should see 2 items in a viewport of size 20 starting at position 25"
        );
        assert_eq!(visible[0].index(), 2, "First visible item should be item 2");
        assert_eq!(
            visible[1].index(),
            3,
            "Second visible item should be item 3"
        );
    }

    #[test]
    fn test_error_handling() {
        let mut list = VirtualList::new(5, 10.0, Orientation::Vertical, 2);

        // Test index out of bounds
        let result = list.update_item_sizes(&[6], &[20.0]);
        assert!(
            result.is_err(),
            "Update with out of bounds index should return error"
        );

        // Test negative size
        let result = list.update_item_sizes(&[1], &[-5.0]);
        assert!(
            result.is_err(),
            "Update with negative size should return error"
        );

        // Test mismatched arrays
        let result = list.update_item_sizes(&[1, 2], &[20.0]);
        assert!(
            result.is_err(),
            "Update with mismatched arrays should return error"
        );

        // Test checked_get_position
        let result = list.checked_get_position(10);
        assert!(
            result.is_err(),
            "checked_get_position with invalid index should return error"
        );
    }

    #[test]
    fn test_binary_search_edge_cases() {
        // Test with very small sizes
        let mut list = VirtualList::new(10, 0.1, Orientation::Vertical, 5);

        // All items are size 0.1, so 10 items total size is 1.0
        let idx = list.find_smallest_i_where_prefix_sum_ge(0.95);
        assert_eq!(
            idx, 9,
            "Should find the correct index even with small sizes"
        );

        // Update to have some zero-sized items
        list.update_item_sizes(&[2, 3, 4], &[0.0, 0.0, 0.0])
            .unwrap();

        let idx = list.find_smallest_i_where_prefix_sum_ge(0.3);
        assert_eq!(idx, 6, "Should handle zero-sized items correctly");
    }

    #[test]
    fn test_single_item_chunk() {
        // Test with each chunk containing just one item
        let list = VirtualList::new(5, 10.0, Orientation::Vertical, 1);

        // Verify chunk structure
        assert_eq!(list.get_position(0), 0.0);
        assert_eq!(list.get_position(1), 10.0);
        assert_eq!(list.get_position(4), 40.0);

        let visible = list.compute_visible_range(15.0, 20.0, 0);
        assert_eq!(visible.len(), 2, "Should see 2 items");
        assert_eq!(visible[0].index(), 2, "First visible item should be item 2");
    }

    #[test]
    fn test_large_chunk() {
        // Test with all items in a single chunk
        let list = VirtualList::new(100, 10.0, Orientation::Vertical, 100);

        // Verify visible range calculation with large chunk
        let visible = list.compute_visible_range(250.0, 50.0, 0);
        assert_eq!(visible.len(), 5, "Should see 5 items with viewport size 50");
        assert_eq!(
            visible[0].index(),
            25,
            "First visible item should be item 25"
        );
    }
}

#[cfg(test)]
mod benchmarks {
    extern crate criterion;
    use super::*;
    use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

    fn bench_initialize(c: &mut Criterion) {
        let mut group = c.benchmark_group("initialization");

        for size in [100, 1000, 10000].iter() {
            group.bench_with_input(BenchmarkId::new("small_chunks", size), size, |b, &size| {
                b.iter(|| VirtualList::new(black_box(size), 10.0, Orientation::Vertical, 10));
            });

            group.bench_with_input(BenchmarkId::new("medium_chunks", size), size, |b, &size| {
                b.iter(|| VirtualList::new(black_box(size), 10.0, Orientation::Vertical, 100));
            });

            group.bench_with_input(BenchmarkId::new("large_chunks", size), size, |b, &size| {
                b.iter(|| VirtualList::new(black_box(size), 10.0, Orientation::Vertical, 1000));
            });
        }

        group.finish();
    }

    fn bench_update_sizes(c: &mut Criterion) {
        let mut group = c.benchmark_group("update_sizes");

        // Prepare different list sizes
        let mut list_small = VirtualList::new(1000, 10.0, Orientation::Vertical, 100);
        let mut list_medium = VirtualList::new(10000, 10.0, Orientation::Vertical, 100);

        // Prepare update batches of different sizes
        let indices_small: Vec<u32> = (0..10).collect();
        let sizes_small: Vec<f64> = (0..10).map(|i| (i as f64) + 5.0).collect();

        let indices_medium: Vec<u32> = (0..100).collect();
        let sizes_medium: Vec<f64> = (0..100).map(|i| (i as f64) + 5.0).collect();

        let indices_scattered: Vec<u32> = (0..50).map(|i| i * 20).collect();
        let sizes_scattered: Vec<f64> = (0..50).map(|i| (i as f64) + 5.0).collect();

        // Benchmark different update patterns
        group.bench_function("small_batch", |b| {
            b.iter(|| {
                list_small
                    .update_item_sizes(black_box(&indices_small), black_box(&sizes_small))
                    .unwrap()
            })
        });

        group.bench_function("medium_batch", |b| {
            b.iter(|| {
                list_medium
                    .update_item_sizes(black_box(&indices_medium), black_box(&sizes_medium))
                    .unwrap()
            })
        });

        group.bench_function("scattered_updates", |b| {
            b.iter(|| {
                list_medium
                    .update_item_sizes(black_box(&indices_scattered), black_box(&sizes_scattered))
                    .unwrap()
            })
        });

        group.finish();
    }

    fn bench_compute_visible(c: &mut Criterion) {
        let mut group = c.benchmark_group("compute_visible");

        // Create different lists to test
        let uniform_list = VirtualList::new(10000, 10.0, Orientation::Vertical, 100);

        let mut variable_list = VirtualList::new(10000, 10.0, Orientation::Vertical, 100);
        // Update every 10th item to have a larger size
        let var_indices: Vec<u32> = (0..1000).map(|i| i * 10).collect();
        let var_sizes: Vec<f64> = (0..1000).map(|_| 50.0).collect();
        variable_list
            .update_item_sizes(&var_indices, &var_sizes)
            .unwrap();

        // Benchmark different viewport scenarios
        group.bench_function("small_viewport_uniform", |b| {
            b.iter(|| {
                uniform_list.compute_visible_range(
                    black_box(5000.0),
                    black_box(100.0),
                    black_box(0),
                )
            })
        });

        group.bench_function("large_viewport_uniform", |b| {
            b.iter(|| {
                uniform_list.compute_visible_range(
                    black_box(5000.0),
                    black_box(1000.0),
                    black_box(0),
                )
            })
        });

        group.bench_function("small_viewport_variable", |b| {
            b.iter(|| {
                variable_list.compute_visible_range(
                    black_box(5000.0),
                    black_box(100.0),
                    black_box(0),
                )
            })
        });

        group.bench_function("with_overscan", |b| {
            b.iter(|| {
                uniform_list.compute_visible_range(
                    black_box(5000.0),
                    black_box(100.0),
                    black_box(10),
                )
            })
        });

        group.finish();
    }

    fn bench_position_queries(c: &mut Criterion) {
        let mut group = c.benchmark_group("position_queries");

        // Create different list configurations
        let small_chunks = VirtualList::new(10000, 10.0, Orientation::Vertical, 10);
        let medium_chunks = VirtualList::new(10000, 10.0, Orientation::Vertical, 100);
        let large_chunks = VirtualList::new(10000, 10.0, Orientation::Vertical, 1000);

        // Benchmark get_position with different chunk sizes
        group.bench_function("get_position_small_chunks", |b| {
            b.iter(|| {
                for i in (0..10000).step_by(100) {
                    black_box(small_chunks.get_position(i));
                }
            })
        });

        group.bench_function("get_position_medium_chunks", |b| {
            b.iter(|| {
                for i in (0..10000).step_by(100) {
                    black_box(medium_chunks.get_position(i));
                }
            })
        });

        group.bench_function("get_position_large_chunks", |b| {
            b.iter(|| {
                for i in (0..10000).step_by(100) {
                    black_box(large_chunks.get_position(i));
                }
            })
        });

        // Benchmark binary search operations
        group.bench_function("binary_search", |b| {
            b.iter(|| {
                for pos in (0..100000).step_by(1000) {
                    black_box(medium_chunks.find_smallest_i_where_prefix_sum_ge(pos as f64));
                }
            })
        });

        group.finish();
    }

    criterion_group!(
        benches,
        bench_initialize,
        bench_update_sizes,
        bench_compute_visible,
        bench_position_queries
    );
    criterion_main!(benches);
}
