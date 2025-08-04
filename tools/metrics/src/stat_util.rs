pub fn median_f64(l: &[f64]) -> f64 {
    // Filter out NaNs
    let mut a: Vec<f64> = l.iter().cloned().filter(|x| !x.is_nan()).collect();

    if a.is_empty() {
        return 0.0;
    }

    a.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let len = a.len();
    if len % 2 == 1 {
        a[len / 2]
    } else {
        (a[len / 2 - 1] + a[len / 2]) / 2.0
    }
}

pub fn mean_f64(l: &[f64]) -> f64 {
    let filtered: Vec<f64> = l.iter().cloned().filter(|x| !x.is_nan()).collect();

    if filtered.is_empty() {
        return 0.0;
    }

    filtered.iter().sum::<f64>() / filtered.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_median_empty() {
        let v: &[f64] = &[];
        assert_eq!(median_f64(v), 0.0);
    }

    #[test]
    fn test_median_single() {
        let v = &[42.0];
        assert_eq!(median_f64(v), 42.0);
    }

    #[test]
    fn test_median_odd() {
        let v = &[3.0, 1.0, 2.0];
        // Sorted: [1.0, 2.0, 3.0], median = 2.0
        assert_eq!(median_f64(v), 2.0);
    }

    #[test]
    fn test_median_even() {
        let v = &[3.0, 1.0, 4.0, 2.0];
        // Sorted: [1.0, 2.0, 3.0, 4.0], median = (2.0 + 3.0) / 2 = 2.5
        assert_eq!(median_f64(v), 2.5);
    }

    #[test]
    fn test_median_nan_multi() {
        let v = &[1.0, f64::NAN, 2.0];
        // Sorted: [1.0, 2.0], median = (1.0 + 2.0) / 2 = 1.5
        assert_eq!(median_f64(v), 1.5);
    }

    #[test]
    fn test_median_nan_single() {
        let v = &[f64::NAN];
        assert_eq!(median_f64(v), 0.0);
    }

    #[test]
    fn test_mean_empty() {
        let v: &[f64] = &[];
        assert_eq!(mean_f64(v), 0.0);
    }

    #[test]
    fn test_mean_single() {
        let v = &[42.0];
        assert_eq!(mean_f64(v), 42.0);
    }

    #[test]
    fn test_mean_multiple() {
        let v = &[1.0, 2.0, 3.0, 4.0];
        assert_eq!(mean_f64(v), 2.5);
    }

    #[test]
    fn test_mean_negative() {
        let v = &[-1.0, 1.0];
        assert_eq!(mean_f64(v), 0.0);
    }
}
