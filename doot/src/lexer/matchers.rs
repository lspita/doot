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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
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

    fn chain(states: Vec<Self>, mut before_switch: impl FnMut(&str, &Self) + 'a) -> Self {
        let mut states = states.into_iter();
        let mut current = None;
        for state in states.by_ref() {
            before_switch("", &state);
            if state.value != MatcherState::Closeable {
                current = Some(state);
                break;
            }
        }
        Self::new(
            match &current {
                Some(s) => s.value.clone(),
                None => MatcherState::Closeable,
            },
            move |buff, ch| match current {
                Some(ref mut c) => {
                    c.accept(buff, ch);
                    match &c.value {
                        MatcherState::Closeable => match states.next() {
                            Some(s) => {
                                before_switch(buff, &s);
                                current = Some(s);
                                current.as_ref().unwrap().value.clone()
                            }
                            None => c.value.clone(),
                        },
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
                    let mut consumed = false;
                    MatcherStateManager::new(MatcherState::Open, move |buff, ch| {
                        if !consumed && c(buff, ch) {
                            consumed = true;
                            MatcherState::Closeable
                        } else {
                            MatcherState::Broken
                        }
                    })
                })
                .collect(),
            |_, _| {},
        )
    }

    fn text(source: &str) -> Self {
        fn make_filter(c: char) -> Box<dyn FnMut(&str, char) -> bool> {
            Box::new(move |_, ch| ch == c)
        }
        Self::conditions(source.chars().map(make_filter).collect())
    }

    fn take_while(mut filter: impl FnMut(&str, char) -> bool + 'a, min: usize) -> Self {
        let mut count = 0;
        let check_count = move |count| {
            if count < min {
                MatcherState::Open
            } else {
                MatcherState::Closeable
            }
        };
        Self::new(check_count(count), move |buff, ch| {
            if filter(buff, ch) {
                count += 1;
                check_count(count)
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
        consume_terminator: bool,
        mut closer: impl FnMut(&str, &str, &mut LexerStateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        let terminators = terminators.map(String::from);
        Self::new(
            MatcherClass::Dynamic,
            MatcherStateManager::filtered_collector(terminators.clone(), filter),
            move |buffer, state| {
                let terminator = terminators.iter().find(|t| buffer.ends_with(*t)).unwrap();
                let value = buffer.strip_suffix(terminator).unwrap();
                closer(value, terminator, state).map(|t| {
                    (
                        t,
                        if consume_terminator {
                            buffer.len()
                        } else {
                            value.len()
                        },
                    )
                })
            },
        )
    }

    pub(super) fn collector<const N: usize>(
        terminators: [&str; N],
        consume_terminator: bool,
        closer: impl FnMut(&str, &str, &mut LexerStateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Self::filtered_collector(terminators, |_, _| true, consume_terminator, closer)
    }

    pub(super) fn take_while(
        filter: impl FnMut(&str, char) -> bool + 'a,
        min: usize,
        closer: impl FnMut(&str, &mut LexerStateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Self::new(
            MatcherClass::Dynamic,
            MatcherStateManager::take_while(filter, min),
            Self::full_match_closer(closer),
        )
    }
}

impl<'a> DefaultMatcher<'a, String> {
    pub(super) fn text_string(value: &str) -> Box<dyn Matcher<String> + 'a> {
        Self::simple_text(value, value.to_string())
    }

    pub(super) fn take_while_string(
        filter: impl FnMut(&str, char) -> bool + 'a,
        min: usize,
    ) -> Box<dyn Matcher<String> + 'a> {
        Self::take_while(filter, min, |value, _| Ok(value.to_string()))
    }
}

pub(super) struct ChainMatcher<'a, T, U: Clone, const N: usize> {
    class: MatcherClass,
    matchers: Rc<RefCell<[Box<dyn Matcher<U> + 'a>; N]>>,
    buffer_indexes: Rc<RefCell<Vec<usize>>>,
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
        let buffer_indexes = Rc::new(RefCell::new(vec![]));
        Box::new(Self {
            class: MatcherClass::Dynamic,
            matchers: matchers.clone(),
            buffer_indexes: buffer_indexes.clone(),
            state: MatcherStateManager::chain(
                (0..len)
                    .map(|i| {
                        let state = matchers.borrow()[i].state().clone();
                        let matchers = matchers.clone();
                        MatcherStateManager::new(state, move |buffer, ch| -> MatcherState {
                            let mut m = matchers.borrow_mut();
                            m[i].accept(buffer, ch);
                            m[i].state().clone()
                        })
                    })
                    .collect(),
                move |buffer, _| {
                    let buffer_indexes: Rc<RefCell<Vec<usize>>> = buffer_indexes.clone();
                    let mut b = buffer_indexes.borrow_mut();
                    b.push(buffer.len());
                },
            ),
            closer: Box::new(closer),
        })
    }

    fn buffer_start_static(buffer_ranges: &RefMut<'_, Vec<usize>>) -> usize {
        buffer_ranges.last().unwrap().clone()
    }

    fn buffer_start(&self) -> usize {
        Self::buffer_start_static(&self.buffer_indexes.borrow_mut())
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
        let buffer_indexes: Vec<usize> = self
            .buffer_indexes
            .borrow_mut()
            .iter()
            .chain([buffer.len()].iter())
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

        fn close<T>(&self, mut matcher: Box<dyn Matcher<T>>) -> usize {
            matcher
                .close(&self.buffer, &mut LexerStateManager::new())
                .unwrap()
                .1
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
        let mut matcher = DefaultMatcher::collector(["01", "23"], false, |value, terminator, _| {
            assert_eq!("abc", value);
            assert_eq!("23", terminator);
            Ok(())
        });
        ctx.matcher_accept('a', &mut matcher);
        ctx.matcher_accept('b', &mut matcher);
        ctx.matcher_accept('c', &mut matcher);
        ctx.matcher_accept('2', &mut matcher);
        ctx.matcher_accept('3', &mut matcher);
        assert_eq!(3, ctx.close(matcher));
    }

    #[rstest]
    fn collector_closer_consume_terminator(mut ctx: Context) {
        let mut matcher = DefaultMatcher::collector(["01", "23"], true, |_, _, _| Ok(()));
        ctx.matcher_accept('a', &mut matcher);
        ctx.matcher_accept('b', &mut matcher);
        ctx.matcher_accept('c', &mut matcher);
        ctx.matcher_accept('2', &mut matcher);
        ctx.matcher_accept('3', &mut matcher);
        assert_eq!(5, ctx.close(matcher));
    }

    #[rstest]
    fn take_while(mut ctx: Context) {
        let mut state = MatcherStateManager::take_while(|_, ch| ch.is_alphanumeric(), 2);
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('d', &mut state); // alphanumeric
        assert_eq!(MatcherState::Open, state.value);
        ctx.state_accept('e', &mut state); // alphanumeric
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept('2', &mut state); // alphanumeric
        assert_eq!(MatcherState::Closeable, state.value);
        ctx.state_accept(' ', &mut state); // expected alphanumeric
        assert_eq!(MatcherState::Broken, state.value);
    }

    #[rstest]
    fn chain(mut ctx: Context) {
        let mut state = MatcherStateManager::chain(
            vec![
                MatcherStateManager::text("abc"),
                MatcherStateManager::filtered_collector(["\t".to_string()], |_, ch| {
                    ch.is_whitespace()
                }),
            ],
            |_, _| {},
        );
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
    fn chain_before_switch(mut ctx: Context) {
        let mut count = 0;
        let mut state = MatcherStateManager::chain(
            vec![
                MatcherStateManager::text("abc"),
                MatcherStateManager::filtered_collector(["\t".to_string()], |_, ch| {
                    ch.is_whitespace()
                }),
            ],
            |buff, _| {
                count += 1;
                match count {
                    1 => assert_eq!("", buff),
                    2 => assert_eq!("abc", buff),
                    _ => panic!(),
                }
            },
        );
        ctx.state_accept('a', &mut state); // 'a'
        ctx.state_accept('b', &mut state); // 'b'
        ctx.state_accept('c', &mut state); // 'c', switch to next
        ctx.state_accept(' ', &mut state); // whitespace
        ctx.state_accept('\t', &mut state); // '\t', terminator
    }

    #[rstest]
    fn chain_closer(mut ctx: Context) {
        let mut matcher = ChainMatcher::new(
            [
                DefaultMatcher::text_string("abc"),
                DefaultMatcher::text_string("def"),
            ],
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

    #[rstest]
    fn chain_closer_take_while(mut ctx: Context) {
        let mut matcher = ChainMatcher::new(
            [
                DefaultMatcher::text_string("abc"),
                DefaultMatcher::take_while(
                    |_, ch| ch.is_alphabetic(),
                    1,
                    |val, _| Ok(val.to_string()),
                ),
            ],
            |value, [first, second], _| {
                assert_eq!("abcdef", value);
                assert_eq!("abc", first);
                assert_eq!("def", second);
                Ok(())
            },
        );
        assert_eq!(MatcherState::Open, *matcher.state());
        ctx.matcher_accept('a', &mut matcher);
        assert_eq!(MatcherState::Open, *matcher.state());
        ctx.matcher_accept('b', &mut matcher);
        assert_eq!(MatcherState::Open, *matcher.state());
        ctx.matcher_accept('c', &mut matcher);
        assert_eq!(MatcherState::Open, *matcher.state());
        ctx.matcher_accept('d', &mut matcher);
        assert_eq!(MatcherState::Closeable, *matcher.state());
        ctx.matcher_accept('e', &mut matcher);
        assert_eq!(MatcherState::Closeable, *matcher.state());
        ctx.matcher_accept('f', &mut matcher);
        assert_eq!(MatcherState::Closeable, *matcher.state());
        ctx.close(matcher);
    }
}
