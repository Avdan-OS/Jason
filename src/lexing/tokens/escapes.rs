//!
//! Escape
//!
//! Technically not tokens.
//! These are used between strings and identifiers.
//!

use avjason_macros::{verbatim as v, Spanned};

use crate::{
    common::{Source, Span},
    lexing::{Exactly, Lex, LexError, LexT, SourceStream},
};

use super::{line_terminator::is_line_terminator, number::HexDigit};
#[derive(Debug, Spanned)]
pub enum EscapeSequence {
    CharacterEscapeSequence(CharacterEscapeSequence),
    Null(Null),
    HexEscapeSequence(HexEscapeSequence),
    UnicodeEscapeSequence(UnicodeEscapeSequence),
}

#[derive(Debug, Spanned)]
pub enum CharacterEscapeSequence {
    Single(SingleEscapeChar),
    NonEscape(NonEscapeChar),
}

#[derive(Debug, Spanned)]
pub struct SingleEscapeChar {
    span: Span,
    raw: char,
}

#[derive(Debug, Spanned)]
pub struct NonEscapeChar {
    span: Span,
    raw: char,
}

#[derive(Debug, Spanned)]
pub struct Null {
    span: Span,
}

#[derive(Debug, Spanned)]
pub struct HexEscapeSequence(v!('x'), Exactly<2, HexDigit>);

#[derive(Debug, Spanned)]
pub struct UnicodeEscapeSequence(v!('u'), Exactly<4, HexDigit>);

// ---

impl LexT for EscapeSequence {
    fn peek<S: Source>(input: &SourceStream<S>) -> bool {
        <CharacterEscapeSequence as LexT>::peek(input)
            || <Null as LexT>::peek(input)
            || <HexEscapeSequence as LexT>::peek(input)
            || <UnicodeEscapeSequence as LexT>::peek(input)
    }

    fn lex<S: Source>(input: &mut SourceStream<S>) -> Result<Self, LexError> {
        // .unwrap_as_result() ok since one of these variants is upcoming.
        input
            .lex()
            .map(Self::CharacterEscapeSequence)
            .or(|| input.lex().map(Self::Null))
            .or(|| input.lex().map(Self::HexEscapeSequence))
            .or(|| input.lex().map(Self::UnicodeEscapeSequence))
            .unwrap_as_result()
    }
}

impl LexT for CharacterEscapeSequence {
    fn peek<S: Source>(input: &SourceStream<S>) -> bool {
        <SingleEscapeChar as LexT>::peek(input) || <NonEscapeChar as LexT>::peek(input)
    }

    fn lex<S: Source>(input: &mut SourceStream<S>) -> Result<Self, LexError> {
        // .unwrap_as_result() ok since Self::peek() -> there is one variant ahead.
        Lex::lex(input)
            .map(Self::Single)
            .or(|| Lex::lex(input).map(Self::NonEscape))
            .unwrap_as_result()
    }
}

fn is_single_escape_char(ch: &char) -> bool {
    matches!(ch, '\'' | '"' | '\\' | 'b' | 'f' | 'n' | 'r' | 't' | 'v')
}

impl LexT for SingleEscapeChar {
    fn peek<S: Source>(input: &SourceStream<S>) -> bool {
        input.upcoming(is_single_escape_char)
    }

    fn lex<S: Source>(input: &mut SourceStream<S>) -> Result<SingleEscapeChar, LexError> {
        // Unwrap ok since Self::peek() -> a character exists.
        let (loc, raw) = input.take().unwrap();

        Ok(Self {
            span: Span::from(loc),
            raw,
        })
    }
}

fn is_escape_char(ch: &char) -> bool {
    is_single_escape_char(ch) || matches!(ch, '0'..='9' | 'x' | 'u')
}

impl LexT for NonEscapeChar {
    fn peek<S: Source>(input: &SourceStream<S>) -> bool {
        input.upcoming(|ch: &char| !(is_line_terminator(ch) || is_escape_char(ch)))
    }

    fn lex<S: Source>(input: &mut SourceStream<S>) -> Result<Self, LexError> {
        // Unwrap ok since Self::peek() -> a character exists.
        let (loc, raw) = input.take().unwrap();

        Ok(Self {
            span: Span::from(loc),
            raw,
        })
    }
}

impl LexT for Null {
    fn peek<S: Source>(input: &SourceStream<S>) -> bool {
        input.upcoming("0") && !matches!(input.peek_n(1), Some('0'..='9'))
    }

    fn lex<S: Source>(input: &mut SourceStream<S>) -> Result<Self, LexError> {
        // .unwrap() ok since Self::peek() -> next character exists.
        let (loc, _) = input.take().unwrap();

        Ok(Self {
            span: Span::from(loc),
        })
    }
}

impl LexT for HexEscapeSequence {
    fn peek<S: Source>(input: &SourceStream<S>) -> bool {
        <v!('x') as LexT>::peek(input)
    }

    fn lex<S: Source>(input: &mut SourceStream<S>) -> Result<Self, LexError> {
        Ok(Self(LexT::lex(input)?, LexT::lex(input)?))
    }
}

impl LexT for UnicodeEscapeSequence {
    fn peek<S: Source>(input: &SourceStream<S>) -> bool {
        <v!('u') as LexT>::peek(input)
    }

    fn lex<S: Source>(input: &mut SourceStream<S>) -> Result<Self, LexError> {
        Ok(Self(LexT::lex(input)?, LexT::lex(input)?))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        common::{file::SourceFile, Source},
        lexing::{
            tokens::escapes::{CharacterEscapeSequence, EscapeSequence, NonEscapeChar},
            Exactly, Lex, Verbatim,
        },
    };

    use super::{HexEscapeSequence, Null, SingleEscapeChar, UnicodeEscapeSequence};

    #[test]
    fn single_escape() {
        let source = SourceFile::dummy_file("'\"\\bfnrtv");
        let input = &mut source.stream();
        let esc: Exactly<9, SingleEscapeChar> = input.lex().expect("Valid parse");
        assert!(matches!(
            &*esc,
            &[
                SingleEscapeChar { raw: '\'', .. },
                SingleEscapeChar { raw: '"', .. },
                SingleEscapeChar { raw: '\\', .. },
                SingleEscapeChar { raw: 'b', .. },
                SingleEscapeChar { raw: 'f', .. },
                SingleEscapeChar { raw: 'n', .. },
                SingleEscapeChar { raw: 'r', .. },
                SingleEscapeChar { raw: 't', .. },
                SingleEscapeChar { raw: 'v', .. },
            ]
        ))
    }

    #[test]
    fn non_escape_char() {
        let source = SourceFile::dummy_file("a!£%*&-=💩");
        let input = &mut source.stream();
        let esc: Exactly<9, NonEscapeChar> = input.lex().expect("Valid parse");
        assert!(matches!(
            &*esc,
            &[
                NonEscapeChar { raw: 'a', .. },
                NonEscapeChar { raw: '!', .. },
                NonEscapeChar { raw: '£', .. },
                NonEscapeChar { raw: '%', .. },
                NonEscapeChar { raw: '*', .. },
                NonEscapeChar { raw: '&', .. },
                NonEscapeChar { raw: '-', .. },
                NonEscapeChar { raw: '=', .. },
                NonEscapeChar { raw: '💩', .. },
            ]
        ))
    }

    #[test]
    fn character_escape_sequence() {
        let source = SourceFile::dummy_file("'\"\\bfnrtva!£%*&-=💩");
        let input = &mut source.stream();
        let esc: Exactly<18, CharacterEscapeSequence> = input.lex().expect("Valid parse");
        assert!(matches!(
            &*esc,
            &[
                CharacterEscapeSequence::Single(SingleEscapeChar { raw: '\'', .. }),
                CharacterEscapeSequence::Single(SingleEscapeChar { raw: '"', .. }),
                CharacterEscapeSequence::Single(SingleEscapeChar { raw: '\\', .. }),
                CharacterEscapeSequence::Single(SingleEscapeChar { raw: 'b', .. }),
                CharacterEscapeSequence::Single(SingleEscapeChar { raw: 'f', .. }),
                CharacterEscapeSequence::Single(SingleEscapeChar { raw: 'n', .. }),
                CharacterEscapeSequence::Single(SingleEscapeChar { raw: 'r', .. }),
                CharacterEscapeSequence::Single(SingleEscapeChar { raw: 't', .. }),
                CharacterEscapeSequence::Single(SingleEscapeChar { raw: 'v', .. }),
                CharacterEscapeSequence::NonEscape(NonEscapeChar { raw: 'a', .. }),
                CharacterEscapeSequence::NonEscape(NonEscapeChar { raw: '!', .. }),
                CharacterEscapeSequence::NonEscape(NonEscapeChar { raw: '£', .. }),
                CharacterEscapeSequence::NonEscape(NonEscapeChar { raw: '%', .. }),
                CharacterEscapeSequence::NonEscape(NonEscapeChar { raw: '*', .. }),
                CharacterEscapeSequence::NonEscape(NonEscapeChar { raw: '&', .. }),
                CharacterEscapeSequence::NonEscape(NonEscapeChar { raw: '-', .. }),
                CharacterEscapeSequence::NonEscape(NonEscapeChar { raw: '=', .. }),
                CharacterEscapeSequence::NonEscape(NonEscapeChar { raw: '💩', .. }),
            ]
        ))
    }

    #[test]
    fn null_char() {
        {
            let source = SourceFile::dummy_file("0");
            let input = &mut source.stream();
            let _: Null = input.lex().expect("Valid parse");
        }

        {
            let source = SourceFile::dummy_file("01");
            let input = &mut source.stream();
            let esc = Null::lex(input);
            assert!(esc.is_nothing())
        }
    }

    #[test]
    fn hex_escape() {
        let source = SourceFile::dummy_file("x20x26x25x3c");
        let input = &mut source.stream();
        let _: Exactly<4, HexEscapeSequence> = input.lex().expect("Valid parse");
    }

    #[test]
    fn unicode_escape() {
        let source = SourceFile::dummy_file("u0000u2AFCu6798u1623");
        let input = &mut source.stream();
        let _: Exactly<4, UnicodeEscapeSequence> = input.lex().expect("Valid parse");
    }

    #[test]
    fn mixed() {
        let source =
            SourceFile::dummy_file("'\"\\bfnrtva!£%*&-=💩0x20x26x25x3cu0000u2AFCu6798u1623");
        let input = &mut source.stream();
        let esc: Exactly<27, EscapeSequence> = input.lex().expect("Valid parse");
        assert!(matches!(
            &*esc,
            &[
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::Single(
                    SingleEscapeChar { raw: '\'', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::Single(
                    SingleEscapeChar { raw: '"', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::Single(
                    SingleEscapeChar { raw: '\\', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::Single(
                    SingleEscapeChar { raw: 'b', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::Single(
                    SingleEscapeChar { raw: 'f', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::Single(
                    SingleEscapeChar { raw: 'n', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::Single(
                    SingleEscapeChar { raw: 'r', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::Single(
                    SingleEscapeChar { raw: 't', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::Single(
                    SingleEscapeChar { raw: 'v', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::NonEscape(
                    NonEscapeChar { raw: 'a', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::NonEscape(
                    NonEscapeChar { raw: '!', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::NonEscape(
                    NonEscapeChar { raw: '£', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::NonEscape(
                    NonEscapeChar { raw: '%', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::NonEscape(
                    NonEscapeChar { raw: '*', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::NonEscape(
                    NonEscapeChar { raw: '&', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::NonEscape(
                    NonEscapeChar { raw: '-', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::NonEscape(
                    NonEscapeChar { raw: '=', .. }
                )),
                EscapeSequence::CharacterEscapeSequence(CharacterEscapeSequence::NonEscape(
                    NonEscapeChar { raw: '💩', .. }
                )),
                EscapeSequence::Null(Null { .. }),
                EscapeSequence::HexEscapeSequence(HexEscapeSequence(Verbatim::<"x"> { .. }, _)),
                EscapeSequence::HexEscapeSequence(HexEscapeSequence(Verbatim::<"x"> { .. }, _)),
                EscapeSequence::HexEscapeSequence(HexEscapeSequence(Verbatim::<"x"> { .. }, _)),
                EscapeSequence::HexEscapeSequence(HexEscapeSequence(Verbatim::<"x"> { .. }, _)),
                EscapeSequence::UnicodeEscapeSequence(UnicodeEscapeSequence(
                    Verbatim::<"u"> { .. },
                    _
                )),
                EscapeSequence::UnicodeEscapeSequence(UnicodeEscapeSequence(
                    Verbatim::<"u"> { .. },
                    _
                )),
                EscapeSequence::UnicodeEscapeSequence(UnicodeEscapeSequence(
                    Verbatim::<"u"> { .. },
                    _
                )),
                EscapeSequence::UnicodeEscapeSequence(UnicodeEscapeSequence(
                    Verbatim::<"u"> { .. },
                    _
                )),
            ]
        ))
    }
}