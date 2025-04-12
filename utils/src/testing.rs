use std::panic::AssertUnwindSafe;

pub fn assert_panics(run: impl FnOnce()) {
    let result = std::panic::catch_unwind(AssertUnwindSafe(run));
    assert!(result.is_err());
}

#[cfg(test)]
mod tests {
    use super::assert_panics;

    #[test]
    fn panics() {
        assert_panics(|| panic!());
    }

    #[test]
    #[should_panic]
    fn not_panics() {
        assert_panics(|| { /*no panic */ });
    }
}
