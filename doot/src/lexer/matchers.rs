use std::{
    cell::{RefCell, RefMut},
    rc::Rc,
};

use super::{TokenizationError, state::LexerStateManager};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum MatcherState {
    Open,
    Broken,
    Closeable,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum MatcherClass {
    Fixed,
    Dynamic,
}

type MatchResult<T> = Result<(T, usize), TokenizationError>;

pub(super) trait Matcher<T> {
    fn class(&self) -> &MatcherClass;
    fn state(&self) -> &MatcherState;
    fn accept(&mut self, buffer: &str, ch: char);
    fn close(&mut self, buffer: &str, state: &mut LexerStateManager) -> MatchResult<T>;
}

struct MatcherStateManager<'a> {
    value: MatcherState,
    op: Box<dyn FnMut(&str, char) -> MatcherState + 'a>,
}

impl<'a> MatcherStateManager<'a> {
    fn accept(&mut self, buffer: &str, ch: char) {
        match self.value {
            MatcherState::Broken => panic!(),
            _ => {
                self.value = if ch == '\0' {
                    MatcherState::Broken
                } else {
                    self.op.as_mut()(buffer, ch)
                }
            }
        }
    }

    fn new(state: MatcherState, op: impl FnMut(&str, char) -> MatcherState + 'a) -> Self {
        Self {
            value: state,
            op: Box::new(op),
        }
    }

    fn chain(states: Vec<Self>) -> Self {
        let mut states = states.into_iter();
        let mut current = states.next();
        Self::new(
            match &current {
                Some(s) => s.value.clone(),
                None => MatcherState::Closeable,
            },
            move |buff, ch| match current {
                Some(ref mut s) => {
                    s.accept(buff, ch);
                    match &s.value {
                        MatcherState::Closeable => {
                            current = states.next();
                            match &current {
                                Some(s) => s.value.clone(),
                                None => MatcherState::Closeable,
                            }
                        }
                        s => s.clone(),
                    }
                }
                None => MatcherState::Broken,
            },
        )
    }

    fn conditions(conditions: Vec<Box<dyn FnMut(&str, char) -> bool + 'a>>) -> Self {
        Self::chain(
            conditions
                .into_iter()
                .map(|mut c| {
                    MatcherStateManager::new(MatcherState::Open, move |buff, ch| {
                        if c(buff, ch) {
                            MatcherState::Closeable
                        } else {
                            MatcherState::Broken
                        }
                    })
                })
                .collect(),
        )
    }

    fn text(source: &str) -> Self {
        fn make_filter(c: char) -> Box<dyn FnMut(&str, char) -> bool> {
            Box::new(move |_, ch| ch == c)
        }
        Self::conditions(source.chars().map(make_filter).collect())
    }

    fn take_while(mut filter: impl FnMut(&str, char) -> bool + 'a) -> Self {
        Self::new(MatcherState::Open, move |buff, ch| {
            if filter(buff, ch) {
                MatcherState::Closeable
            } else {
                MatcherState::Broken
            }
        })
    }

    fn filtered_collector<const N: usize>(
        terminators: [String; N],
        mut filter: impl FnMut(&str, char) -> bool + 'a,
    ) -> Self {
        let mut terminated = false;
        Self::new(MatcherState::Open, move |buff, ch| {
            if !terminated && filter(buff, ch) {
                terminated = terminators.iter().any(|t| buff.ends_with(t));
                if terminated {
                    MatcherState::Closeable
                } else {
                    MatcherState::Open
                }
            } else {
                MatcherState::Broken
            }
        })
    }
}

pub(super) struct DefaultMatcher<'a, T> {
    class: MatcherClass,
    state: MatcherStateManager<'a>,
    closer: Box<dyn FnMut(&str, &mut LexerStateManager) -> MatchResult<T> + 'a>,
}

impl<T> Matcher<T> for DefaultMatcher<'_, T> {
    fn class(&self) -> &MatcherClass {
        &self.class
    }

    fn state(&self) -> &MatcherState {
        &self.state.value
    }

    fn accept(&mut self, buffer: &str, ch: char) {
        self.state.accept(buffer, ch);
    }

    fn close(&mut self, buffer: &str, state: &mut LexerStateManager) -> MatchResult<T> {
        self.closer.as_mut()(buffer, state)
    }
}

impl<'a, T: 'a + Clone> DefaultMatcher<'a, T> {
    fn new(
        class: MatcherClass,
        state: MatcherStateManager<'a>,
        closer: impl FnMut(&str, &mut LexerStateManager) -> MatchResult<T> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Box::new(Self {
            class,
            state,
            closer: Box::new(closer),
        })
    }

    fn full_match_closer(
        mut closer: impl FnMut(&str, &mut LexerStateManager) -> Result<T, TokenizationError>,
    ) -> impl FnMut(&str, &mut LexerStateManager) -> MatchResult<T> {
        move |buff, state| {
            let len = buff.len();
            closer(buff, state).map(move |t| (t, len))
        }
    }

    pub(super) fn conditions(
        conditions: Vec<Box<dyn FnMut(&str, char) -> bool + 'a>>,
        closer: impl FnMut(&str, &mut LexerStateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Self::new(
            MatcherClass::Dynamic,
            MatcherStateManager::conditions(conditions),
            Self::full_match_closer(closer),
        )
    }

    pub(super) fn text(
        source: &str,
        closer: impl FnMut(&str, &mut LexerStateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Self::new(
            MatcherClass::Fixed,
            MatcherStateManager::text(source),
            Self::full_match_closer(closer),
        )
    }

    pub(super) fn simple_text(source: &str, result: T) -> Box<dyn Matcher<T> + 'a> {
        Self::text(source, move |_, _| Ok(result.clone()))
    }

    pub(super) fn filtered_collector<const N: usize>(
        terminators: [&str; N],
        filter: impl FnMut(&str, char) -> bool + 'a,
        mut closer: impl FnMut(&str, &str, &mut LexerStateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        let terminators = terminators.map(String::from);
        Self::new(
            MatcherClass::Dynamic,
            MatcherStateManager::filtered_collector(terminators.clone(), filter),
            move |buffer, state| {
                let terminator = terminators.iter().find(|t| buffer.ends_with(*t)).unwrap();
                let value = buffer.strip_suffix(terminator).unwrap();
                closer(value, terminator, state).map(|t| (t, value.len()))
            },
        )
    }

    pub(super) fn collector<const N: usize>(
        terminators: [&str; N],
        closer: impl FnMut(&str, &str, &mut LexerStateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Self::filtered_collector(terminators, |_, _| true, closer)
    }

    pub(super) fn take_while(
        filter: impl FnMut(&str, char) -> bool + 'a,
        closer: impl FnMut(&str, &mut LexerStateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Self::new(
            MatcherClass::Dynamic,
            MatcherStateManager::take_while(filter),
            Self::full_match_closer(closer),
        )
    }
}

impl<'a> DefaultMatcher<'a, String> {
    pub(super) fn prefix(value: &str) -> Box<dyn Matcher<String> + 'a> {
        Self::simple_text(value, value.to_string())
    }
}

pub(super) struct ChainMatcher<'a, T, U: Clone, const N: usize> {
    class: MatcherClass,
    matchers: Rc<RefCell<[Box<dyn Matcher<U> + 'a>; N]>>,
    buffer_ranges: Rc<RefCell<Vec<usize>>>,
    state: MatcherStateManager<'a>,
    closer:
        Box<dyn FnMut(&str, [U; N], &mut LexerStateManager) -> Result<T, TokenizationError> + 'a>,
}

impl<'a, T: 'a, U: 'a + Clone, const N: usize> ChainMatcher<'a, T, U, N> {
    pub(super) fn new(
        matchers: [Box<dyn Matcher<U>>; N],
        closer: impl FnMut(&str, [U; N], &mut LexerStateManager) -> Result<T, TokenizationError>
        + 'a + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        let len = matchers.len();
        let matchers = Rc::new(RefCell::new(matchers));
        let buffer_ranges = Rc::new(RefCell::new(vec![]));
        Box::new(Self {
            class: MatcherClass::Dynamic,
            matchers: Rc::clone(&matchers),
            buffer_ranges: Rc::clone(&buffer_ranges),
            state: MatcherStateManager::chain(
                (0..len)
                    .map(|i| {
                        let state = matchers.borrow()[i].state().clone();
                        let matchers = Rc::clone(&matchers);
                        let buffer_ranges = Rc::clone(&buffer_ranges);
                        MatcherStateManager::new(state, move |buffer, ch| -> MatcherState {
                            let mut m = matchers.borrow_mut();
                            let mut b = buffer_ranges.borrow_mut();
                            m[i].accept(buffer, ch);
                            let state = m[i].state();
                            if *state == MatcherState::Closeable {
                                let start = Self::buffer_start_static(&b);
                                b.push(buffer.len() + start);
                            }
                            state.clone()
                        })
                    })
                    .collect(),
            ),
            closer: Box::new(closer),
        })
    }

    fn buffer_start_static(buffer_ranges: &RefMut<'_, Vec<usize>>) -> usize {
        buffer_ranges.last().unwrap_or(&0).clone()
    }

    fn buffer_start(&self) -> usize {
        Self::buffer_start_static(&self.buffer_ranges.borrow_mut())
    }
}

impl<'a, T: 'a, U: Clone + 'a, const N: usize> Matcher<T> for ChainMatcher<'a, T, U, N> {
    fn class(&self) -> &MatcherClass {
        &self.class
    }

    fn state(&self) -> &MatcherState {
        &self.state.value
    }

    fn accept(&mut self, buffer: &str, ch: char) {
        let start = self.buffer_start();
        self.state.accept(&buffer[start..], ch);
    }

    fn close(&mut self, buffer: &str, state: &mut LexerStateManager) -> MatchResult<T> {
        let mut matchers = self.matchers.borrow_mut();
        let buffer_indexes: Vec<usize> = [0]
            .iter()
            .chain(self.buffer_ranges.borrow_mut().iter())
            .map(usize::clone)
            .collect();
        let results = std::array::from_fn(|i| {
            matchers[i].close(&buffer[buffer_indexes[i]..buffer_indexes[i + 1]], state)
        });
        if let Some(err) = results.iter().find_map(|r| r.as_ref().err()) {
            Err(err.clone())
        } else {
            self.closer.as_mut()(
                &buffer,
                results.map(|r| r.clone().unwrap()).map(|r| r.0),
                state,
            )
            .map(|t| (t, buffer.len()))
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::{fixture, rstest};
    use utils::testing::assert_panics;

    use crate::lexer::{
        LexerStateManager, MatcherState,
        matchers::{ChainMatcher, DefaultMatcher},
    };

    use super::{Matcher, MatcherStateManager};

    struct Context {
        buffer: String,
    }

    impl<'a> Context {
        fn setup() -> Self {
            Self {
                buffer: String::new(),
            }
        }

        fn state_accept(&mut self, ch: char, state: &mut MatcherStateManager) {
            self.buffer.push(ch);
            state.accept(&self.buffer, ch);
        }

        fn matcher_accept<T>(&mut self, ch: char, matcher: &mut Box<dyn Matcher<T>>) {
            self.buffer.push(ch);
            matcher.accept(&self.buffer, ch);
        }

        fn close<T>(&self, mut matcher: Box<dyn Matcher<T>>) {
            let _ = matcher.close(&self.buffer, &mut LexerStateManager::new());
        }
    }

    #[fixture]
    fn ctx() -> Context {
        Context::setup()
    }

    #[rstest]
    fn stop_accepting_after_break() {
        let mut i = 0;
        let mut state = MatcherStateManager::new(MatcherState::Open, |_, _| {
            i += 1;
            if i == 3 {
                MatcherState::Broken
            } else {
                MatcherState::Open
            }
        });
        assert_eq!(MatcherState::Open, state.value);
        state.accept("", 'a'); // 0 -> 1
        assert_eq!(MatcherState::Open, state.value);
        state.accept("", 'a'); // 1 -> 2
        assert_eq!(MatcherState::Open, state.value);
        state.accept("", 'a'); // 2 -> 3
        assert_eq!(MatcherState::Broken, state.value);
        assert_panics(|| state.accept("", 'a')); // panic accepting when broken
    }

    #[rstest]
    fn break_with_null_char() {
        let mut state =
            MatcherStateManager::new(MatcherState::Open, |_, _| MatcherState::Closeable);
        assert_eq!(MatcherState::Open, state.value);
        state.accept("", '\0');
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn conditions_ok(mut ctx: Context) {
        let mut state = MatcherStateManager::conditions(vec![
            Box::new(|_, ch| ch.is_alphanumeric()),
            Box::new(|_, ch| ch.is_whitespace()),
            Box::new(|buff, _| buff.contains('a')),
        ]);
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('a', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept(' ', &mut state); // whitespace
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('c', &mut state); // buffer already contains 'a'
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept('d', &mut state); // any char, it will break because of no conditions left
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn conditions_fail(mut ctx: Context) {
        let mut state = MatcherStateManager::conditions(vec![
            Box::new(|_, ch| ch.is_alphanumeric()),
            Box::new(|_, ch| ch.is_whitespace()),
        ]);
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('a', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('b', &mut state); // expected whitespace
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn text_ok(mut ctx: Context) {
        let mut state = MatcherStateManager::text("hello");
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('h', &mut state); // 'h'
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('e', &mut state); // 'e'
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('l', &mut state); // 'l'
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('l', &mut state); // 'l'
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('o', &mut state); // 'o'
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept('0', &mut state); // any char, it will break because of no text left
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn text_empty(mut ctx: Context) {
        let mut state = MatcherStateManager::text("");
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept('h', &mut state); // any char, it will break because of no text left
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn text_fail(mut ctx: Context) {
        let mut state = MatcherStateManager::text("hello");
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('h', &mut state); // 'h'
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('b', &mut state); // expected 'e'
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn collector_single_terminator(mut ctx: Context) {
        let mut state = MatcherStateManager::filtered_collector(["01".to_string()], |_, ch| {
            ch.is_alphanumeric()
        });
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('a', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('1', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('c', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('0', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('1', &mut state); // alphanumeric, "01" terminator
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept('d', &mut state); // any char, it will break because terminator has been found
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn collector_multiple_terminators(mut ctx: Context) {
        fn create_state<'a>() -> MatcherStateManager<'a> {
            MatcherStateManager::filtered_collector(
                ["01".to_string(), "23".to_string()],
                |_, ch| ch.is_alphanumeric(),
            )
        }
        let mut state = create_state();
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('a', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('1', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('c', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('0', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('1', &mut state); // alphanumeric, "01" terminator
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept('d', &mut state); // any char, it will break because terminator has been found
        assert_eq!(MatcherState::Broken, state.value);

        ctx = Context::setup();
        let mut state = create_state();
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('d', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('e', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('2', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('3', &mut state); // alphanumeric, "23" terminator
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept('d', &mut state); // any char, it will break because terminator has been found
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn collector_fail_filter(mut ctx: Context) {
        let mut state = MatcherStateManager::filtered_collector(["01".to_string()], |_, ch| {
            ch.is_alphanumeric()
        });
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('a', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept(' ', &mut state); // expected alphanumeric
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn collector_closer(mut ctx: Context) {
        let mut matcher = DefaultMatcher::collector(["01", "23"], |value, terminator, _| {
            assert_eq!("abc", value);
            assert_eq!("23", terminator);
            Ok(())
        });
        ctx.matcher_accept('a', &mut matcher);
        ctx.matcher_accept('b', &mut matcher);
        ctx.matcher_accept('c', &mut matcher);
        ctx.matcher_accept('2', &mut matcher);
        ctx.matcher_accept('3', &mut matcher);
        ctx.close(matcher);
    }

    #[rstest]
    fn take_while(mut ctx: Context) {
        let mut state = MatcherStateManager::take_while(|_, ch| ch.is_alphanumeric());
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('d', &mut state); // alphanumeric
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept('e', &mut state); // alphanumeric
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept('2', &mut state); // alphanumeric
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept(' ', &mut state); // expected alphanumeric
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn chain(mut ctx: Context) {
        let mut state = MatcherStateManager::chain(vec![
            MatcherStateManager::text("abc"),
            MatcherStateManager::filtered_collector(["\t".to_string()], |_, ch| ch.is_whitespace()),
        ]);
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('a', &mut state); // 'a'
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('b', &mut state); // 'b'
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('c', &mut state); // 'c', switch to next
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept(' ', &mut state); // whitespace
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('\t', &mut state); // '\t', terminator
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept(' ', &mut state); // any char, it will break because all closed
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn chain_closer(mut ctx: Context) {
        let mut matcher = ChainMatcher::new(
            [DefaultMatcher::prefix("abc"), DefaultMatcher::prefix("def")],
            |value, [first, second], _| {
                assert_eq!("abcdef", value);
                assert_eq!("abc", first);
                assert_eq!("def", second);
                Ok(())
            },
        );
        ctx.matcher_accept('a', &mut matcher);
        ctx.matcher_accept('b', &mut matcher);
        ctx.matcher_accept('c', &mut matcher);
        ctx.matcher_accept('d', &mut matcher);
        ctx.matcher_accept('e', &mut matcher);
        ctx.matcher_accept('f', &mut matcher);
        ctx.close(matcher);
    }
}
