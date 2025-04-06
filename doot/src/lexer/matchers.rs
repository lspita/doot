use std::{cell::RefCell, rc::Rc};

use super::{TokenizationError, state::StateManager};

#[derive(Clone)]
pub(super) enum State {
    Open,
    Broken,
    Closeable,
}

type MatchResult<T> = Result<(T, usize), TokenizationError>;

pub(super) trait Matcher<T> {
    fn buffer(&self) -> String;
    fn state(&self) -> &State;
    fn accept(&mut self, ch: char);
    fn close(&mut self, state: &mut StateManager) -> MatchResult<T>;
}

struct MatcherState<'a> {
    value: State,
    op: Box<dyn FnMut(&str, char) -> State + 'a>,
}

impl<'a> MatcherState<'a> {
    fn accept(&mut self, buffer: &str, ch: char) {
        match self.value {
            State::Open => {
                self.value = self.op.as_mut()(buffer, ch);
            }
            _ => {}
        }
    }

    fn new(state: State, op: impl FnMut(&str, char) -> State + 'a) -> Self {
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
                None => State::Closeable,
            },
            move |buff, ch| match current {
                Some(ref mut s) => {
                    s.accept(buff, ch);
                    match &s.value {
                        State::Closeable => {
                            current = states.next();
                            match &current {
                                Some(s) => s.value.clone(),
                                None => State::Closeable,
                            }
                        }
                        s => s.clone(),
                    }
                }
                None => State::Broken,
            },
        )
    }

    fn conditions(conditions: Vec<Box<dyn FnMut(&str, char) -> bool>>) -> Self {
        Self::chain(
            conditions
                .into_iter()
                .map(|mut c| {
                    MatcherState::new(State::Open, move |buff, ch| {
                        if c.as_mut()(buff, ch) {
                            State::Closeable
                        } else {
                            State::Broken
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

    fn filtered_collector<const N: usize>(
        terminators: [String; N],
        mut filter: impl FnMut(&str, char) -> bool + 'a,
    ) -> Self {
        Self::new(State::Open, move |buffer, ch| {
            if terminators.iter().any(|t| buffer.ends_with(t)) {
                State::Closeable
            } else if !filter(&buffer, ch) {
                State::Broken
            } else {
                State::Open
            }
        })
    }

    fn take_while(mut filter: impl FnMut(&str, char) -> bool + 'a, min: usize) -> Self {
        let mut count = 0;
        Self::new(State::Open, move |buff, ch| {
            if filter(buff, ch) {
                count += 1;
                State::Open
            } else {
                if count < min {
                    State::Broken
                } else {
                    State::Closeable
                }
            }
        })
    }
}

pub(super) struct BufferedMatcher<'a, T> {
    state: MatcherState<'a>,
    buffer: String,
    closer: Box<dyn FnMut(&str, &mut StateManager) -> MatchResult<T> + 'a>,
}

impl<T> Matcher<T> for BufferedMatcher<'_, T> {
    fn buffer(&self) -> String {
        self.buffer.clone()
    }

    fn state(&self) -> &State {
        &self.state.value
    }

    fn accept(&mut self, ch: char) {
        self.state.accept(&self.buffer, ch);
    }

    fn close(&mut self, state: &mut StateManager) -> MatchResult<T> {
        match self.state.value {
            State::Closeable => self.closer.as_mut()(&self.buffer, state),
            _ => panic!(),
        }
    }
}

impl<'a, T: 'a + Clone> BufferedMatcher<'a, T> {
    fn new(
        state: MatcherState<'a>,
        closer: impl FnMut(&str, &mut StateManager) -> MatchResult<T> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Box::new(Self {
            state,
            buffer: String::new(),
            closer: Box::new(closer),
        })
    }

    fn full_match_closer(
        mut closer: impl FnMut(&str, &mut StateManager) -> Result<T, TokenizationError>,
    ) -> impl FnMut(&str, &mut StateManager) -> MatchResult<T> {
        move |buff, state| {
            let len = buff.len();
            closer(buff, state).map(move |t| (t, len))
        }
    }

    pub(super) fn conditions(
        conditions: Vec<Box<dyn FnMut(&str, char) -> bool>>,
        closer: impl FnMut(&str, &mut StateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Self::new(
            MatcherState::conditions(conditions),
            Self::full_match_closer(closer),
        )
    }

    pub(super) fn text(
        source: &str,
        closer: impl FnMut(&str, &mut StateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Self::new(MatcherState::text(source), Self::full_match_closer(closer))
    }

    pub(super) fn simple_text(source: &str, result: T) -> Box<dyn Matcher<T> + 'a> {
        Self::text(source, move |_, _| Ok(result.clone()))
    }

    pub(super) fn filtered_collector<const N: usize>(
        terminators: [&str; N],
        filter: impl FnMut(&str, char) -> bool + 'a,
        mut closer: impl FnMut(&str, &str, &mut StateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        let terminators = terminators.map(String::from);
        Self::new(
            MatcherState::filtered_collector(terminators.clone(), filter),
            move |buffer, state| {
                let terminator = terminators.iter().find(|t| buffer.ends_with(*t)).unwrap();
                let value = buffer.strip_suffix(terminator).unwrap();
                closer(value, terminator, state).map(|t| (t, value.len()))
            },
        )
    }

    pub(super) fn collector<const N: usize>(
        terminators: [&str; N],
        closer: impl FnMut(&str, &str, &mut StateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Self::filtered_collector(terminators, |_, _| true, closer)
    }

    pub(super) fn take_while(
        filter: impl FnMut(&str, char) -> bool + 'a,
        min: usize,
        closer: impl FnMut(&str, &mut StateManager) -> Result<T, TokenizationError> + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        Self::new(
            MatcherState::take_while(filter, min),
            Self::full_match_closer(closer),
        )
    }
}

impl<'a> BufferedMatcher<'a, String> {
    pub(super) fn prefix(value: &str) -> Box<dyn Matcher<String> + 'a> {
        Self::simple_text(value, value.to_string())
    }
}

pub(super) struct ChainMatcher<'a, T, U: Clone, const N: usize> {
    matchers: Rc<RefCell<[Box<dyn Matcher<U> + 'a>; N]>>,
    state: MatcherState<'a>,
    closer: Box<dyn FnMut(&str, [U; N], &mut StateManager) -> Result<T, TokenizationError> + 'a>,
}

impl<T, U: Clone, const N: usize> Matcher<T> for ChainMatcher<'_, T, U, N> {
    fn buffer(&self) -> String {
        self.matchers.borrow().iter().map(|m| m.buffer()).collect()
    }

    fn state(&self) -> &State {
        &self.state.value
    }

    fn accept(&mut self, ch: char) {
        self.state.accept(&self.buffer(), ch);
    }

    fn close(&mut self, state: &mut StateManager) -> MatchResult<T> {
        let buffer = self.buffer();
        let mut matchers = self.matchers.borrow_mut();
        let results = std::array::from_fn(|i| matchers[i].close(state));
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

impl<'a, T: 'a, U: 'a + Clone, const N: usize> ChainMatcher<'a, T, U, N> {
    pub(super) fn new(
        matchers: [Box<dyn Matcher<U>>; N],
        closer: impl FnMut(&str, [U; N], &mut StateManager) -> Result<T, TokenizationError> + 'a + 'a,
    ) -> Box<dyn Matcher<T> + 'a> {
        let len = matchers.len();
        let matchers = Rc::new(RefCell::new(matchers));
        Box::new(Self {
            matchers: Rc::clone(&matchers),
            state: MatcherState::chain(
                (0..len)
                    .map({
                        let matchers = Rc::clone(&matchers);
                        move |i| {
                            let state = matchers.borrow()[i].state().clone();
                            let matchers = Rc::clone(&matchers);
                            MatcherState::new(state, move |_, ch| -> State {
                                let mut m = matchers.borrow_mut();
                                m[i].accept(ch);
                                m[i].state().clone()
                            })
                        }
                    })
                    .collect(),
            ),
            closer: Box::new(closer),
        })
    }
}
