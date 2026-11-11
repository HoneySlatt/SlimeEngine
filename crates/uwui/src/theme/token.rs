//! Tokens: the themes' values named in code, each of a kind, with a fallback
//! (`docs/adr/0238-tokens-typed-named-in-code-declared-by-each-crate.md`).
//!
//! A token names a value that themes give under its name, `magnolia_knob_face`, and says of what
//! kind it is: a [`Color`], a [`Length`] in logical pixels, a [`Duration`], or a [`Number`]. Code
//! writes it where it would write a literal of its kind, `s.width(MAGNOLIA_KNOB_SIZE)`; the
//! interface reads its value once, when it takes its themes ([`Ui::set_style`]), and styles the
//! element with it. A change of theme styles every element again and runs no view.
//!
//! A crate lists its tokens ([`Tokens`]), and a program declares each list to its interface once
//! ([`Ui::declare_tokens`]): the interface then reads each token in its kind, refuses themes that
//! give it a value of another kind, and tells once of a token that the themes lack, which takes its
//! fallback. The core's roles are tokens that every interface declares ([`crate::theme`]).
//!
//! ```
//! use uwui::theme::{self, Color, Fallback, Length, Token, Tokens};
//! use uwui::tree::Element;
//!
//! pub const MAGNOLIA_KNOB_FACE: Token<Color> =
//!     Token::new("magnolia_knob_face", Fallback::Srgb(0x2A2D33));
//! pub const MAGNOLIA_KNOB_RING: Token<Color> =
//!     Token::new("magnolia_knob_ring", Fallback::Token(&theme::ACCENT));
//! pub const MAGNOLIA_KNOB_SIZE: Token<Length> =
//!     Token::new("magnolia_knob_size", Fallback::Value(28.0));
//! pub const MAGNOLIA_TOKENS: Tokens =
//!     Tokens::new(&[&MAGNOLIA_KNOB_FACE, &MAGNOLIA_KNOB_RING, &MAGNOLIA_KNOB_SIZE]);
//!
//! let knob = Element::button("Gain").style(|s| {
//!     s.width(MAGNOLIA_KNOB_SIZE)
//!         .background(MAGNOLIA_KNOB_FACE)
//!         .border_color(MAGNOLIA_KNOB_RING)
//! });
//! # let _ = knob;
//! ```
//!
//! A setter takes a token of its own kind alone: a duration where a length goes does not compile.
//!
//! ```compile_fail,E0277
//! use uwui::theme::{Duration, Fallback, Token};
//! use uwui::tree::Element;
//!
//! const FAST: Token<Duration> =
//!     Token::new("fast", Fallback::Value(std::time::Duration::from_millis(120)));
//! let wrong = Element::button("Save").style(|s| s.padding_top(FAST));
//! ```
//!
//! [`Ui::set_style`]: crate::tree::Ui::set_style
//! [`Ui::declare_tokens`]: crate::tree::Ui::declare_tokens

use std::fmt;

use slime_color::LinearColor;

/// The kind of a token: what its value is, and how a theme writes it. Implemented by [`Color`],
/// [`Length`], [`Duration`] and [`Number`] alone.
pub trait TokenKind: sealed::Sealed + Copy + fmt::Debug + Send + Sync + 'static {
    /// The value of a token of this kind.
    type Value: Copy + fmt::Debug + PartialEq + Send + Sync + 'static;
}

/// A color, a `LinearColor`, which a theme writes as a color: `"#rrggbb"`, a value's name or a
/// color function.
#[derive(Clone, Copy, Debug)]
pub struct Color;

/// A length in logical pixels, an `f32`, which a theme writes as a number: a padding, a radius, a
/// size of text, or a size, which it gives as [`Dimension::Pixels`](crate::style::Dimension).
#[derive(Clone, Copy, Debug)]
pub struct Length;

/// A time, a [`std::time::Duration`], which a theme writes as a number of seconds, never negative:
/// how long a transition takes. It never changes with the density.
#[derive(Clone, Copy, Debug)]
pub struct Duration;

/// A number that is neither a length nor a time, an `f32`: an opacity, a weight, a share.
#[derive(Clone, Copy, Debug)]
pub struct Number;

impl TokenKind for Color {
    type Value = LinearColor;
}

impl TokenKind for Length {
    type Value = f32;
}

impl TokenKind for Duration {
    type Value = std::time::Duration;
}

impl TokenKind for Number {
    type Value = f32;
}

impl sealed::Sealed for Color {
    const KIND: sealed::KindId = sealed::KindId::Color;

    fn wrap(value: LinearColor) -> sealed::Resolved {
        sealed::Resolved::Color(value)
    }

    fn unwrap(resolved: sealed::Resolved) -> Option<LinearColor> {
        match resolved {
            sealed::Resolved::Color(value) => Some(value),
            _ => None,
        }
    }
}

impl sealed::Sealed for Length {
    const KIND: sealed::KindId = sealed::KindId::Length;

    fn wrap(value: f32) -> sealed::Resolved {
        sealed::Resolved::Length(value)
    }

    fn unwrap(resolved: sealed::Resolved) -> Option<f32> {
        match resolved {
            sealed::Resolved::Length(value) => Some(value),
            _ => None,
        }
    }
}

impl sealed::Sealed for Duration {
    const KIND: sealed::KindId = sealed::KindId::Duration;

    fn wrap(value: std::time::Duration) -> sealed::Resolved {
        sealed::Resolved::Duration(value)
    }

    fn unwrap(resolved: sealed::Resolved) -> Option<std::time::Duration> {
        match resolved {
            sealed::Resolved::Duration(value) => Some(value),
            _ => None,
        }
    }
}

impl sealed::Sealed for Number {
    const KIND: sealed::KindId = sealed::KindId::Number;

    fn wrap(value: f32) -> sealed::Resolved {
        sealed::Resolved::Number(value)
    }

    fn unwrap(resolved: sealed::Resolved) -> Option<f32> {
        match resolved {
            sealed::Resolved::Number(value) => Some(value),
            _ => None,
        }
    }
}

/// The value that a token takes when the themes do not give it.
#[derive(Clone, Copy, Debug)]
pub enum Fallback<K: TokenKind> {
    /// A value of its kind; for a color, in linear light.
    Value(K::Value),
    /// A color written as sRGB's `0xRRGGBB`, as a theme writes `"#RRGGBB"`, decoded when the
    /// interface reads its themes: a token of another kind given one, or a number past
    /// `0xFFFFFF`, does not compile in a `const`.
    Srgb(u32),
    /// The value of another token of its kind, whatever gives it: the themes or its own fallback.
    Token(&'static Token<K>),
}

/// A value of the themes named in code, of the kind `K`, with the value it takes when the themes
/// do not give it. Its name is how themes give it and how the interface knows it: two tokens of one
/// name are one token. A token is written as a `const`, so that a cycle of fallbacks between
/// tokens does not compile.
#[derive(Clone, Copy, Debug)]
pub struct Token<K: TokenKind> {
    name: &'static str,
    fallback: Fallback<K>,
}

impl<K: TokenKind> Token<K> {
    /// The token that themes give under `name`, `fallback` when they do not. An application's own
    /// tokens take its name as a prefix, `magnolia_`, so that they never meet another crate's.
    ///
    /// # Panics
    ///
    /// For an empty name, a fallback given as sRGB to a token that is not a color, or as sRGB past
    /// `0xFFFFFF`: in a `const`, an error of compiling.
    #[must_use]
    pub const fn new(name: &'static str, fallback: Fallback<K>) -> Self {
        assert!(!name.is_empty(), "a token's name is not empty");
        if let Fallback::Srgb(hex) = fallback {
            assert!(
                matches!(<K as sealed::Sealed>::KIND, sealed::KindId::Color),
                "only a color's fallback is written in sRGB"
            );
            assert!(hex <= 0xFF_FFFF, "an sRGB fallback is `0xRRGGBB`");
        }
        Self { name, fallback }
    }

    /// The name that themes give it under.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The value it takes when the themes do not give it.
    #[must_use]
    pub const fn fallback(&self) -> Fallback<K> {
        self.fallback
    }

    /// What an own style keeps of it: its name, kind and fallback, any kind alike.
    pub(crate) fn erased(&self) -> Erased {
        Erased {
            name: self.name,
            kind: <K as sealed::Sealed>::KIND,
            fallback: match self.fallback {
                Fallback::Value(value) => ErasedFallback::Value(<K as sealed::Sealed>::wrap(value)),
                Fallback::Srgb(hex) => ErasedFallback::Srgb(hex),
                Fallback::Token(token) => ErasedFallback::Token(token),
            },
        }
    }
}

/// A token of any kind, as a list holds it ([`Tokens`]): implemented by [`Token`] alone.
pub trait AnyToken: sealed::Erase + fmt::Debug + Sync {}

impl<K: TokenKind> AnyToken for Token<K> {}

impl<K: TokenKind> sealed::Erase for Token<K> {
    fn erase(&self) -> Erased {
        self.erased()
    }
}

/// The tokens that a crate declares, a list of tokens of any kind, given to an interface by
/// [`Ui::declare_tokens`](crate::tree::Ui::declare_tokens).
#[derive(Clone, Copy, Debug)]
pub struct Tokens(&'static [&'static dyn AnyToken]);

impl Tokens {
    /// The list of `tokens`, in their order.
    #[must_use]
    pub const fn new(tokens: &'static [&'static dyn AnyToken]) -> Self {
        Self(tokens)
    }

    /// Its tokens, in order.
    pub(crate) fn each(&self) -> impl Iterator<Item = Erased> + '_ {
        self.0.iter().map(|token| token.erase())
    }
}

/// Why an interface refuses a list of tokens ([`Ui::declare_tokens`](crate::tree::Ui::declare_tokens)).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TokenError {
    /// The list declares a name that another token already declared, of another kind or another
    /// fallback: a value is replaced by a theme, not declared again.
    DeclaredTwice {
        /// The name declared twice.
        name: &'static str,
    },
    /// The list declares a token whose fallbacks, each naming the next token, come back to it:
    /// tokens written as `static`, which the compiler does not stop.
    FallbackCycle {
        /// The token whose fallbacks come back to it.
        name: &'static str,
    },
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeclaredTwice { name } => write!(
                f,
                "the token `{name}` is declared twice: a theme replaces a token's value, and an \
                 application's own tokens take its name as a prefix"
            ),
            Self::FallbackCycle { name } => write!(
                f,
                "the fallbacks of the token `{name}` name tokens that come back to it: a token is \
                 written as a `const`"
            ),
        }
    }
}

impl std::error::Error for TokenError {}

/// A literal or a token of the kind `K`, which a setter of an own style takes: `padding_top(12.0)`
/// or `padding_top(SPACE_M)`.
#[derive(Clone, Copy, Debug)]
pub enum TokenOr<T, K: TokenKind> {
    /// The value written in code, the author's own.
    Literal(T),
    /// A value of the themes.
    Token(Token<K>),
}

impl<T, K: TokenKind> From<Token<K>> for TokenOr<T, K> {
    fn from(token: Token<K>) -> Self {
        Self::Token(token)
    }
}

/// The literals that each kind's setters take.
macro_rules! literals {
    ($($ty:ty => $kind:ty),* $(,)?) => {
        $(impl From<$ty> for TokenOr<$ty, $kind> {
            fn from(value: $ty) -> Self {
                Self::Literal(value)
            }
        })*
    };
}

literals!(
    LinearColor => Color,
    f32 => Length,
    crate::style::Dimension => Length,
    crate::style::Sides => Length,
    std::time::Duration => Duration,
    f32 => Number,
);

/// The same space on every side, as [`Sides::all`](crate::style::Sides::all) gives it.
impl From<f32> for TokenOr<crate::style::Sides, Length> {
    fn from(space: f32) -> Self {
        Self::Literal(crate::style::Sides::all(space))
    }
}

pub(crate) use sealed::{KindId, Resolved};

/// What an own style and a list keep of a token, any kind alike: `pub` in a module that is not, so
/// that the sealed traits name it, and never reached from outside uwui.
#[derive(Clone, Copy, Debug)]
pub struct Erased {
    pub(crate) name: &'static str,
    pub(crate) kind: KindId,
    pub(crate) fallback: ErasedFallback,
}

/// A token's fallback, any kind alike.
#[derive(Clone, Copy, Debug)]
pub enum ErasedFallback {
    Value(Resolved),
    Srgb(u32),
    Token(&'static dyn AnyToken),
}

impl Erased {
    /// Whether `other` is the same token: of its name, its kind and its fallback, its value
    /// compared by its bits, a token that it names by that token's name and kind.
    pub(crate) fn same(&self, other: &Self) -> bool {
        self.name == other.name
            && self.kind == other.kind
            && match (self.fallback, other.fallback) {
                (ErasedFallback::Value(a), ErasedFallback::Value(b)) => a.same(&b),
                (ErasedFallback::Srgb(a), ErasedFallback::Srgb(b)) => a == b,
                (ErasedFallback::Token(a), ErasedFallback::Token(b)) => {
                    let (a, b) = (a.erase(), b.erase());
                    a.name == b.name && a.kind == b.kind
                }
                _ => false,
            }
    }

    /// Its fallback's value, where the themes do not give it: a token that it names takes the value
    /// that `known` gives under its name and kind, else its own fallback, and so on. A chain longer
    /// than [`FALLBACK_CHAIN`], a cycle between `static` tokens, ends on a value of its kind that
    /// holds nothing.
    pub(crate) fn fallback_value(
        &self,
        known: &dyn Fn(&'static str, KindId) -> Option<Resolved>,
    ) -> Resolved {
        let mut token = *self;
        for _ in 0..FALLBACK_CHAIN {
            match token.fallback {
                ErasedFallback::Value(value) => return value,
                ErasedFallback::Srgb(hex) => return Resolved::Color(srgb(hex)),
                ErasedFallback::Token(next) => {
                    let next = next.erase();
                    if let Some(value) = known(next.name, next.kind) {
                        return value;
                    }
                    token = next;
                }
            }
        }
        self.kind.placeholder()
    }

    /// Whether its fallbacks, each naming the next token, come back to a token already met, or go
    /// on past [`FALLBACK_CHAIN`].
    pub(crate) fn falls_back_in_a_cycle(&self) -> bool {
        let mut met = vec![self.name];
        let mut token = *self;
        while let ErasedFallback::Token(next) = token.fallback {
            token = next.erase();
            if met.contains(&token.name) || met.len() >= FALLBACK_CHAIN {
                return true;
            }
            met.push(token.name);
        }
        false
    }
}

/// The most tokens that a chain of fallbacks follows.
const FALLBACK_CHAIN: usize = 64;

/// The color of sRGB's `0xRRGGBB`, opaque, decoded as a theme's `"#RRGGBB"` is.
pub(crate) fn srgb(hex: u32) -> LinearColor {
    let [_, r, g, b] = hex.to_be_bytes();
    LinearColor::from_srgb8(r, g, b)
}

mod sealed {
    use slime_color::LinearColor;

    use crate::style::Same;

    /// Keeps [`TokenKind`](super::TokenKind) to uwui's four kinds, and gives what the interface reads of
    /// each.
    pub trait Sealed {
        /// Which kind it is.
        const KIND: KindId;

        /// `value` as a value of any kind.
        fn wrap(value: <Self as super::TokenKind>::Value) -> Resolved
        where
            Self: super::TokenKind;

        /// The value of its kind that `resolved` holds; none for another kind's.
        fn unwrap(resolved: Resolved) -> Option<<Self as super::TokenKind>::Value>
        where
            Self: super::TokenKind;
    }

    /// Keeps [`AnyToken`](super::AnyToken) to uwui's tokens, and erases their kind.
    pub trait Erase {
        fn erase(&self) -> super::Erased;
    }

    /// The four kinds, as values.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum KindId {
        Color,
        Length,
        Duration,
        Number,
    }

    impl KindId {
        /// Its name, as an error gives it.
        pub fn name(self) -> &'static str {
            match self {
                Self::Color => "color",
                Self::Length => "length",
                Self::Duration => "duration",
                Self::Number => "number",
            }
        }

        /// A value of its kind that holds nothing, which an own style's declaration holds in place
        /// of a token's until the token is read: its leaf, which the mask order reads, is the same.
        pub fn placeholder(self) -> Resolved {
            match self {
                Self::Color => Resolved::Color(LinearColor::rgba(0.0, 0.0, 0.0, 0.0)),
                Self::Length => Resolved::Length(0.0),
                Self::Duration => Resolved::Duration(std::time::Duration::ZERO),
                Self::Number => Resolved::Number(0.0),
            }
        }
    }

    /// A token's value, any kind alike.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum Resolved {
        Color(LinearColor),
        Length(f32),
        Duration(std::time::Duration),
        Number(f32),
    }

    impl Resolved {
        /// Its kind.
        pub fn kind(self) -> KindId {
            match self {
                Self::Color(_) => KindId::Color,
                Self::Length(_) => KindId::Length,
                Self::Duration(_) => KindId::Duration,
                Self::Number(_) => KindId::Number,
            }
        }

        /// Whether `other` is the same, its floats compared by their bits.
        pub fn same(&self, other: &Self) -> bool {
            match (self, other) {
                (Self::Color(a), Self::Color(b)) => a.same(b),
                (Self::Length(a), Self::Length(b)) | (Self::Number(a), Self::Number(b)) => {
                    a.same(b)
                }
                (Self::Duration(a), Self::Duration(b)) => a == b,
                _ => false,
            }
        }
    }
}
