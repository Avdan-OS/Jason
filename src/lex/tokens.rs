use avjason_macros::{Lex, Spanned};
use finl_unicode::categories::{CharacterCategories, MinorCategory};

use crate::utils::{SourceIter, Span, TryIntoSpan};

use super::{escape::UnicodeEscapeSequence, number::Number, strings::LString, IntoLexResult};

pub(crate) trait Lex: Sized {
    fn lex(input: &mut SourceIter) -> impl IntoLexResult<Self>;
    fn peek(input: &SourceIter) -> bool;
}

#[derive(Debug, Spanned)]
#[Lex('{')]
pub struct OpenBrace(Span);

#[derive(Debug, Spanned)]
#[Lex('}')]
pub struct CloseBrace(Span);

#[derive(Debug, Spanned)]
#[Lex('[')]
pub struct OpenBracket(Span);

#[derive(Debug, Spanned)]
#[Lex(']')]
pub struct CloseBracket(Span);

#[derive(Debug, Spanned)]
#[Lex(':')]
pub struct Colon(Span);

#[derive(Debug, Spanned)]
#[Lex(',')]
pub struct Comma(Span);

#[derive(Debug, Spanned)]
#[Lex('-')]
pub struct Minus(Span);

#[derive(Debug, Spanned)]
#[Lex('+')]
pub struct Plus(Span);

#[derive(Debug, Spanned)]
#[Lex('.')]
pub struct Dot(Span);

#[macro_export]
macro_rules! Token {
    ['{'] => {
        $crate::lex::tokens::OpenBrace
    };
    ['}'] => {
        $crate::lex::tokens::CloseBrace
    };
    ['['] => {
        $crate::lex::tokens::OpenBracket
    };
    [']'] => {
        $crate::lex::tokens::CloseBracket
    };
    [':'] => {
        $crate::lex::tokens::Colon
    };
    [','] => {
        $crate::lex::tokens::Comma
    };
    ['-'] => {
        $crate::lex::tokens::Minus
    };
    ['+'] => {
        $crate::lex::tokens::Plus
    };
    ['.'] => {
        $crate::lex::tokens::Dot
    };
    [:] => {
        $crate::lex::tokens::Colon
    };
    [,] => {
        $crate::lex::tokens::Comma
    };
    [-] => {
        $crate::lex::tokens::Minus
    };
    [+] => {
        $crate::lex::tokens::Plus
    };
    [.] => {
        $crate::lex::tokens::Dot
    };
}

#[derive(Debug, Spanned)]
#[Lex]
pub enum Punct {
    OpenBrace(OpenBrace),
    CloseBrace(CloseBrace),
    OpenBracket(OpenBracket),
    CloseBracket(CloseBracket),
    Colon(Colon),
    Comma(Comma),
}

#[derive(Debug, Spanned)]
pub struct WhiteSpace(Span);

impl WhiteSpace {
    ///
    /// In accordance with
    /// [ECMAScript standards](https://262.ecma-international.org/5.1/#sec-7.2).
    ///
    pub fn is_whitespace(ch: &char) -> bool {
        ch == &'\u{0009}'
            || ch == &'\u{000b}'
            || ch == &'\u{000c}'
            || ch == &'\u{0020}'
            || ch == &'\u{00a0}'
            || (*ch).get_minor_category() == MinorCategory::Zs
    }
}

impl Lex for WhiteSpace {
    fn lex(input: &mut SourceIter) -> Option<Self> {
        let ch = input.peek()?;
        let Some(start) = (if Self::is_whitespace(ch) {
            Some(input.next()?.0)
        } else {
            return None;
        }) else {
            return None;
        };

        let mut end = start;
        while let Some(ch) = input.peek() {
            if !Self::is_whitespace(ch) {
                break;
            }
            end = input.next()?.0;
        }

        Some(Self(TryIntoSpan::try_into_span(start..=end)?))
    }

    fn peek(input: &SourceIter) -> bool {
        input.peek().map(Self::is_whitespace).unwrap_or_default()
    }
}

///
/// In accordance with the [ECMAScript standard](https://262.ecma-international.org/5.1/#sec-7.3).
///
#[derive(Debug, Spanned)]
pub struct LineTerminator(Span);

impl Lex for LineTerminator {
    fn lex(input: &mut SourceIter) -> Option<Self> {
        match input.peek()? {
            // <LF>, <CR>, <LS>, <PS>
            &'\u{000a}' | &'\u{000d}' | &'\u{2028}' | &'\u{2029}' => {
                let loc = input.next()?.0;
                Some(Self(Span::single_char(loc)))
            }
            _ => None,
        }
    }

    fn peek(input: &SourceIter) -> bool {
        matches!(
            input.peek(),
            Some(&'\u{000a}' | &'\u{000d}' | &'\u{2028}' | &'\u{2029}')
        )
    }
}

#[derive(Debug, Spanned)]
pub struct LineTerminatorSeq(Span);

impl Lex for LineTerminatorSeq {
    fn lex(input: &mut SourceIter) -> Option<Self> {
        match (input.peek()?, input.peek2()) {
            // <CR><LF>
            (&'\u{000d}', Some(&'\u{000a}')) => {
                let start = input.next()?.0;
                let end = input.next()?.0;
                Some(Self(TryIntoSpan::try_into_span(start..=end)?))
            }
            // <LF>, <CR>, <LS>, <PS>
            (&'\u{000a}' | &'\u{000d}' | &'\u{2028}' | &'\u{2029}', _) => {
                let loc = input.next()?.0;
                Some(Self(Span::single_char(loc)))
            }
            _ => None,
        }
    }

    fn peek(input: &SourceIter) -> bool {
        match (input.peek(), input.peek2()) {
            // <CR><LF>
            (Some(&'\u{000d}'), Some(&'\u{000a}')) => true,
            // <LF>, <CR>, <LS>, <PS>
            (Some(&'\u{000a}' | &'\u{000d}' | &'\u{2028}' | &'\u{2029}'), _) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Spanned)]
#[Lex]
pub enum Comment {
    MultiLine(MultiLineComment),
    SingleLine(SingleLineComment),
}

#[derive(Debug, Spanned)]
pub struct SingleLineComment(Span);

impl Lex for SingleLineComment {
    fn lex(input: &mut SourceIter) -> Option<Self> {
        if !Self::peek(input) {
            return None;
        }

        let start = input.next()?.0; // First slash
        let _ = input.next()?; // Second slash

        let mut end = start;
        while !LineTerminator::peek(input) {
            // Unwrap ok since peek -> Some implies next -> Some/
            end = input.next().unwrap().0;
        }

        Some(Self(TryIntoSpan::try_into_span(start..=end)?))
    }

    fn peek(input: &SourceIter) -> bool {
        matches!((input.peek(), input.peek2()), (Some(&'/'), Some(&'/')))
    }
}

#[derive(Debug, Spanned)]
pub struct MultiLineComment(Span);

impl MultiLineComment {
    fn peek_end(input: &SourceIter) -> bool {
        matches!((input.peek(), input.peek2()), (Some(&'*'), Some(&'/')))
    }
}

impl Lex for MultiLineComment {
    fn lex(input: &mut SourceIter) -> Option<Self> {
        if !Self::peek(input) {
            return None;
        }

        let start = input.next()?.0; // First slash
        let _ = input.next()?; // Second slash

        while !Self::peek_end(input) {
            // Unwrap ok since peek -> Some implies next -> Some
            _ = input.next().unwrap().0;
        }

        input.next().unwrap(); // `*` - Unwraps ok since peek, peek2 -> Some, Some
        let end = input.next().unwrap().0; // `/`

        Some(Self(TryIntoSpan::try_into_span(start..=end)?))
    }

    fn peek(input: &SourceIter) -> bool {
        matches!((input.peek(), input.peek2()), (Some(&'/'), Some(&'*')))
    }
}

#[derive(Debug, Spanned)]
#[Lex]
pub enum InputElement {
    LineTerminator(LineTerminator),
    WhiteSpace(WhiteSpace),
    Comment(Comment),
    Token(Token),
}

///
/// Compliant with [ECMAScript specification for `IdentifierName`](https://262.ecma-international.org/5.1/#sec-7.6).
///
#[derive(Debug, Spanned)]
pub struct LIdentifier(Span);

impl LIdentifier {
    fn is_unicode_letter(ch: &char) -> bool {
        use MinorCategory::*;
        matches!(ch.get_minor_category(), Lu | Ll | Lt | Lm | Lo | Nl)
    }

    fn is_unicode_combining_mark(ch: &char) -> bool {
        use MinorCategory::*;
        matches!(ch.get_minor_category(), Mn | Mc)
    }

    fn is_unicode_digit(ch: &char) -> bool {
        use MinorCategory::*;
        matches!(ch.get_minor_category(), Nd)
    }

    fn is_unicode_connector_punctuation(ch: &char) -> bool {
        use MinorCategory::*;
        matches!(ch.get_minor_category(), Pc)
    }

    pub(crate) fn is_identifier_start(input: &SourceIter) -> bool {
        // IdentifierStart
        let Some(ch) = input.peek() else {
            return false;
        };

        match ch {
            c if Self::is_unicode_letter(c) => true,
            &'$' | &'_' => true,
            &'\\' => {
                // Check for unicode escape sequence.
                let mut fork = input.fork();
                fork.next().unwrap();
                UnicodeEscapeSequence::peek(input)
            }
            _ => false,
        }
    }

    fn is_identifier_part(input: &SourceIter) -> bool {
        if Self::is_identifier_start(input) {
            return true;
        }

        let Some(ch) = input.peek() else {
            return false;
        };

        Self::is_unicode_combining_mark(ch)
            || Self::is_unicode_digit(ch)
            || Self::is_unicode_connector_punctuation(ch)
            || matches!(ch, &'\u{200c}' | &'\u{200d}') // <ZWNJ> | <ZWJ>
    }

    fn peek_middle(input: &SourceIter) -> bool {
        Self::is_identifier_part(input)
    }
}

impl Lex for LIdentifier {
    fn lex(input: &mut SourceIter) -> Option<Self> {
        if !Self::peek(input) {
            return None;
        }

        let start = input.next().unwrap().0;
        let mut end = start + 1;
        while Self::peek_middle(input) {
            end = input.next().unwrap().0;
        }

        Some(Self(TryIntoSpan::try_into_span(start..=end)?))
    }

    fn peek(input: &SourceIter) -> bool {
        Self::is_identifier_start(input)
    }
}

#[derive(Debug, Spanned)]
pub enum Token {
    Identifier(LIdentifier),
    Punctuator(Punct),
    String(LString),
    Number(Number),
}

impl Lex for Token {
    fn lex(input: &mut SourceIter) -> impl IntoLexResult<Self> {
        if let Some(s) = LIdentifier::lex(input).into_lex_result()? {
            return Ok(Some(Self::Identifier(s)));
        }

        if let Some(s) = Punct::lex(input).into_lex_result()? {
            return Ok(Some(Self::Punctuator(s)));
        }

        if let Some(s) = LString::lex(input).into_lex_result()? {
            return Ok(Some(Self::String(s)));
        }

        if let Some(s) = Number::lex(input).into_lex_result()? {
            return Ok(Some(Self::Number(s)));
        }

        Ok(None)
    }

    fn peek(_: &SourceIter) -> bool {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        lex::IntoLexResult,
        utils::SourceFile,
    };

    use super::{InputElement, Lex};

    #[test]
    fn lexxing_tests() {
        let src = "\
        []\n\
        21, 5.65
        {     }:,\n\
        // Single line comment\n\
        /* Multi line Comment\n\
        Wa-hey!*/\r\n
        \"Here's a string!\"\n
        1.234678\t7.2367\t-Infinity";

        println!("{src:?}");
        let src = SourceFile::dummy_file("test.1", src);
        let iter = &mut src.iter();
        while let Ok(Some(l)) = InputElement::lex(iter).into_lex_result() {
            println!("--> {l:?}");
        }
    }
}