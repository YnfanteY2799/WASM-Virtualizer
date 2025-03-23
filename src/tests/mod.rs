#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_empty_list() {
        let list = VirtualList::new(0, 10.0, Orientation::Vertical, 10);
        let visible = list.compute_visible_range(0.0, 100.0, 0);
        assert_eq!(
            visible.len(),
            0,
            "Empty list should return no visible items"
        );
    }

    #[test]
    fn test_single_item() {
        let list = VirtualList::new(1, 50.0, Orientation::Vertical, 1);
        let visible = list.compute_visible_range(0.0, 100.0, 0);
        assert_eq!(visible.len(), 1, "Should return one visible item");
        assert_eq!(visible[0].index, 0, "Index should be 0");
        assert_eq!(visible[0].position, 0.0, "Position should be 0.0");
    }

    #[test]
    fn test_update_item_sizes_success() {
        let mut list = VirtualList::new(3, 10.0, Orientation::Vertical, 2);
        let result = list.update_item_sizes(&[0, 1], &[20.0, 30.0]);
        assert!(result.is_ok(), "Update should succeed with valid inputs");
    }

    #[test]
    fn test_update_item_sizes_invalid_length() {
        let mut list = VirtualList::new(2, 10.0, Orientation::Vertical, 1);
        let result = list.update_item_sizes(&[0], &[10.0, 20.0]);
        assert!(result.is_err(), "Should fail with mismatched lengths");
    }

    #[test]
    fn test_update_item_sizes_negative_size() {
        let mut list = VirtualList::new(2, 10.0, Orientation::Vertical, 1);
        let result = list.update_item_sizes(&[0], &[-5.0]);
        assert!(result.is_err(), "Should fail with negative size");
    }

    #[test]
    fn test_update_item_sizes_out_of_bounds() {
        let mut list = VirtualList::new(2, 10.0, Orientation::Vertical, 1);
        let result = list.update_item_sizes(&[2], &[15.0]);
        assert!(result.is_err(), "Should fail with out-of-bounds index");
    }
}
