use crate::{
    characters::{Choseong, JongToCho, Jongseong, Jungseong, KeyValue},
    Addon, InputResult, LayoutContext,
};

/// 한글 입력 오토마타
#[derive(Debug, Clone)]
pub struct HangulState {
    state: CharacterState,
    word_commit: bool,
    commit_buf: String,
    commit_characters: Vec<CharacterState>,
    buf: String,
}

impl HangulState {
    pub fn new(word_commit: bool) -> Self {
        Self {
            state: CharacterState::new(),
            word_commit,
            commit_buf: String::with_capacity(64),
            commit_characters: Vec::with_capacity(16),
            buf: String::with_capacity(64),
        }
    }

    pub fn commit_str(&mut self) -> &str {
        if self.word_commit {
            self.buf.clear();
            self.buf.push_str(&self.commit_buf);
            self.state.write(&mut self.buf);
            &self.buf
        } else {
            &self.commit_buf
        }
    }

    pub fn preedit_str(&mut self) -> &str {
        if self.word_commit {
            self.commit_str()
        } else {
            self.buf.clear();
            self.state.write(&mut self.buf);
            &self.buf
        }
    }

    pub fn pass(&mut self, s: &str) {
        self.commit_characters.clear();
        self.clear_preedit();
        self.commit_buf.push_str(s);
    }

    pub fn pass_replace(&mut self, s: &str) {
        self.commit_characters.clear();

        if self.word_commit {
            self.commit_buf.clear();
        }

        self.state.reset();
        self.commit_buf.push_str(s);
    }

    pub fn clear_preedit(&mut self) {
        self.state.write(&mut self.commit_buf);
        self.state.reset();
    }

    pub fn flush(&mut self) {
        self.commit_characters.clear();
        self.commit_buf.clear();
    }

    pub fn reset(&mut self) {
        self.flush();
        self.state.reset();
    }

    pub fn preedit_result(&self) -> InputResult {
        if self.state.need_display() || self.word_commit && !self.commit_characters.is_empty() {
            InputResult::HAS_PREEDIT
        } else {
            InputResult::empty()
        }
    }

    fn convert_result(&mut self, ret: CharacterResult) -> InputResult {
        match ret {
            CharacterResult::Consume => self.preedit_result() | InputResult::CONSUMED,
            CharacterResult::NewCharacter(new) => {
                self.commit_buf.push(self.state.to_char());
                self.commit_characters
                    .push(std::mem::replace(&mut self.state, new));

                if self.word_commit {
                    InputResult::HAS_PREEDIT | InputResult::CONSUMED
                } else {
                    InputResult::NEED_FLUSH | self.preedit_result() | InputResult::CONSUMED
                }
            }
        }
    }

    pub fn backspace(&mut self, addons: EnumSet<Addon>) -> InputResult {
        loop {
            if self.state.backspace(addons) {
                return self.preedit_result() | InputResult::CONSUMED;
            }

            match self.commit_characters.pop() {
                Some(new_last) => {
                    self.commit_buf.pop();
                    self.state = new_last;
                }
                None => {
                    return InputResult::empty();
                }
            }
        }
    }

    pub fn key(&mut self, kv: &KeyValue, addons: EnumSet<Addon>) -> InputResult {
        let ret = match kv {
            KeyValue::Pass(pass) => {
                self.pass(pass);
                return InputResult::NEED_RESET | InputResult::CONSUMED;
            }
            KeyValue::Choseong { cho } => self.state.cho(*cho, addons),
            KeyValue::Jungseong { jung, compose } => self.state.jung(*jung, *compose, addons),
            KeyValue::Jongseong { jong } => self.state.jong(*jong, addons),
            KeyValue::ChoJong { cho, jong, first } => {
                self.state.cho_jong(*cho, *jong, *first, addons)
            }
            KeyValue::ChoJung {
                cho,
                jung,
                first,
                compose,
            } => self
                .state
                .cho_jung(*cho, *jung, *first, *compose, addons),
            KeyValue::JungJong {
                jung,
                jong,
                first,
                compose,
            } => self
                .state
                .jung_jong(*jung, *jong, *first, *compose, addons),
        };

        self.convert_result(ret)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CharacterResult {
    Consume,
    NewCharacter(CharacterState),
}

/// 한글 글자 상태
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
struct CharacterState {
    cho: Option<Choseong>,
    jung: Option<Jungseong>,
    /// 조합용 중성
    compose_jung: bool,
    jong: Option<Jongseong>,
}

impl CharacterState {
    pub const fn new() -> Self {
        Self {
            cho: None,
            jung: None,
            compose_jung: false,
            jong: None,
        }
    }

    pub fn reset(&mut self) {
        self.cho = None;
        self.jung = None;
        self.jong = None;
    }

    pub fn to_char(&self) -> char {
        match (self.cho, self.jung, self.jong) {
            (None, None, None) |
            // can't be char
            (None, Some(_), Some(_)) |
            (Some(_), None, Some(_)) => '\0',

            (Some(cho), Some(jung), jong) => cho.compose(jung, jong),

            (Some(cho), None, None) => cho.jamo(),
            (None, Some(jung), None) => jung.jamo(),
            (None, None, Some(jong)) => jong.jamo(),
        }
    }

    pub fn write(&self, out: &mut String) {
        let ch = self.to_char();

        if ch != '\0' {
            out.push(ch);
        }
    }

    pub const fn need_display(&self) -> bool {
        match (self.cho, self.jung, self.jong) {
            (None, None, None) |
            // can't be char
            (None, Some(_), Some(_)) |
            (Some(_), None, Some(_)) => false,
            _ => true,
        }
    }

    pub fn backspace(&mut self, addons: EnumSet<Addon>) -> bool {
        if let Some(jong) = self.jong.as_mut() {
            if let Some(new_jong) = jong.backspace(addons) {
                *jong = new_jong;
            } else {
                self.jong = None;
            }
        } else if let Some(jung) = self.jung.as_mut() {
            if let Some(new_jung) = jung.backspace(addons) {
                *jung = new_jung;
                self.compose_jung = true;
            } else {
                self.jung = None;
                self.compose_jung = false;
            }
        } else if let Some(cho) = self.cho.as_mut() {
            if let Some(new_cho) = cho.backspace(addons) {
                *cho = new_cho;
            } else {
                self.cho = None;
            }
        } else {
            // empty state
            return false;
        }

        true
    }

    fn choseong_can_compose_jongseong(&self, cho: Choseong, addons: EnumSet<Addon>) -> bool {
        self.jong.map_or(false, |j| match j.to_cho(addons) {
            JongToCho::Direct(prev_cho) | JongToCho::Compose(_, prev_cho) => {
                prev_cho.try_add(cho, addons).is_some()
            }
        })
    }

    // 갈마들이 입력

    pub fn cho_jong(
        &mut self,
        cho: Choseong,
        jong: Jongseong,
        first: bool,
        addons: EnumSet<Addon>,
    ) -> CharacterResult {
        if self.cho.is_none()
            || self.jung.is_none()
            || addons.contains(Addon::TreatJongseongAsChoseongCompose)
                && self.choseong_can_compose_jongseong(cho, addons)
        {
            self.cho(cho, addons)
        } else if self.jung.is_some() || !first {
            self.jong(jong, addons)
        } else {
            self.cho(cho, addons)
        }
    }

    pub fn cho_jung(
        &mut self,
        cho: Choseong,
        jung: Jungseong,
        first: bool,
        compose_jung: bool,
        addons: EnumSet<Addon>,
    ) -> CharacterResult {
        if self.cho.is_some()
            && self.jung.map_or(true, |j| {
                self.compose_jung && j.try_add(jung, addons).is_some()
            })
        {
            self.jung(jung, compose_jung, addons)
        } else if self.cho.is_none() || first {
            self.cho(cho, addons)
        } else {
            self.jung(jung, compose_jung, addons)
        }
    }

    pub fn jung_jong(
        &mut self,
        jung: Jungseong,
        jong: Jongseong,
        first: bool,
        compose_jung: bool,
        addons: EnumSet<Addon>,
    ) -> CharacterResult {
        // 아 + $ㄴㅖ = 안
        // ㅇ + $ㅜ + $ㅊㅔ = 웨
        // ㅇ + ㅜ + $ㅊㅔ = 웇
        if self.jung.map_or(true, |j| {
            self.compose_jung && j.try_add(jung, addons).is_some()
        }) {
            self.jung(jung, compose_jung, addons)
        } else if self.cho.is_some() || !first {
            self.jong(jong, addons)
        } else {
            self.jung(jung, compose_jung, addons)
        }
    }

    // 일반 입력

    pub fn cho(&mut self, mut cho: Choseong, addons: EnumSet<Addon>) -> CharacterResult {
        if let Some(prev_cho) = self.cho {
            if let Some(jong) = self.jong {
                if addons.contains(Addon::TreatJongseongAsChoseongCompose) {
                    match jong.to_cho(addons) {
                        JongToCho::Direct(prev_cho) => {
                            if let Some(new_cho) = prev_cho.try_add(cho, addons) {
                                self.jong = None;
                                cho = new_cho;
                            }
                        }
                        JongToCho::Compose(jong, prev_cho) => {
                            if let Some(new_cho) = prev_cho.try_add(cho, addons) {
                                self.jong = Some(jong);
                                cho = new_cho;
                            }
                        }
                    }
                }

                CharacterResult::NewCharacter(Self {
                    cho: Some(cho),
                    ..Default::default()
                })
            } else {
                match prev_cho.try_add(cho, addons) {
                    Some(new)
                        if addons.contains(Addon::FlexibleComposeOrder)
                            || self.jung.is_none() =>
                    {
                        self.cho = Some(new);
                        CharacterResult::Consume
                    }
                    _ => CharacterResult::NewCharacter(Self {
                        cho: Some(cho),
                        ..Default::default()
                    }),
                }
            }
        } else if addons.contains(Addon::FlexibleComposeOrder)
            || self.jung.is_none() && self.jong.is_none()
        {
            self.cho = Some(cho);
            CharacterResult::Consume
        } else {
            CharacterResult::NewCharacter(Self {
                cho: Some(cho),
                ..Default::default()
            })
        }
    }

    pub fn jung(
        &mut self,
        jung: Jungseong,
        compose_jung: bool,
        addons: EnumSet<Addon>,
    ) -> CharacterResult {
        if addons.contains(Addon::TreatJongseongAsChoseong) {
            if let Some(jong) = self.jong {
                if self.cho.is_some() {
                    // has choseong move jongseong to next choseong
                    let new;

                    match jong.to_cho(addons) {
                        JongToCho::Direct(cho) => {
                            self.jong = None;
                            new = Self {
                                cho: Some(cho),
                                jung: Some(jung),
                                jong: None,
                                compose_jung,
                            };
                        }
                        JongToCho::Compose(jong, cho) => {
                            self.jong = Some(jong);
                            new = Self {
                                cho: Some(cho),
                                jung: Some(jung),
                                jong: None,
                                compose_jung,
                            };
                        }
                    }

                    return CharacterResult::NewCharacter(new);
                } else {
                    // only jongseong commit replace with jungseong
                    return CharacterResult::NewCharacter(Self {
                        cho: None,
                        jung: Some(jung),
                        jong: None,
                        compose_jung,
                    });
                }
            }
        }

        if let Some(prev_jung) = self.jung {
            match prev_jung.try_add(jung, addons) {
                Some(new) if self.compose_jung => {
                    self.jung = Some(new);
                    self.compose_jung = false;
                    CharacterResult::Consume
                }
                _ => CharacterResult::NewCharacter(Self {
                    jung: Some(jung),
                    compose_jung,
                    ..Default::default()
                }),
            }
        } else {
            self.jung = Some(jung);
            self.compose_jung = compose_jung;
            CharacterResult::Consume
        }
    }

    pub fn jong(&mut self, jong: Jongseong, addons: EnumSet<Addon>) -> CharacterResult {
        if let Some(prev_jong) = self.jong {
            match prev_jong.try_add(jong, addons) {
                Some(new) => {
                    self.jong = Some(new);
                    CharacterResult::Consume
                }
                None => {
                    let new;

                    match jong.to_cho(addons) {
                        JongToCho::Direct(cho) => {
                            new = Self {
                                cho: Some(cho),
                                ..Default::default()
                            };
                        }
                        JongToCho::Compose(..) => {
                            new = Self {
                                jong: Some(jong),
                                ..Default::default()
                            };
                        }
                    }

                    CharacterResult::NewCharacter(new)
                }
            }
        } else {
            self.jong = Some(jong);
            CharacterResult::Consume
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jong() {
        let mut state = CharacterState::default();
        let mut config = crate::Config::default();
        config.default_category = crate::InputCategory::Hangul;
        let addons = LayoutContext::new(&config);

        state.cho_jong(Choseong::Ieung, Jongseong::Ieung, true, &addons);
        state.jung(Jungseong::A, true, &addons);
        state.cho_jong(Choseong::Ieung, Jongseong::Ieung, true, &addons);

        assert_eq!(
            CharacterResult::NewCharacter(CharacterState {
                cho: Some(Choseong::Ieung),
                jung: Some(Jungseong::A),
                compose_jung: true,
                jong: None
            }),
            state.jung(Jungseong::A, true, &addons)
        );
    }
}
