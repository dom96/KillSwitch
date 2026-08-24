use chumsky::{
    input::{Stream, ValueInput},
    prelude::*,
    span::SimpleSpan,
};
use logos::Logos;
use std::fmt;

use crate::lexer::Token;

pub type Span = std::ops::Range<usize>;
pub type Spanned<N> = (N, Span);

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Story(String, Vec<Spanned<Self>>),
    Chapter(String, Vec<Spanned<Self>>),
    FuncCall(String, usize), // ident, number of words
    IntLiteral(i64),
    FloatLiteral(f64),
    FuncReturn,
    ValueRef(usize), // count of "lines below"
    Adverb(String),  // can be referenced by ValueRef
    Word,
    VariableAssign(String),
    VariableRead(String),
    Hack(String, String),
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Story(ident, nodes) => {
                write!(f, "Story({}, ", ident)?;
                for (i, (node, _span)) in nodes.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", node)?;
                }
                write!(f, ")")
            }
            Node::Chapter(ident, nodes) => {
                write!(f, "Chapter({}, ", ident)?;
                for (i, (node, _span)) in nodes.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", node)?;
                }
                write!(f, ")")
            }
            Node::FuncCall(ident, words) => write!(f, "FuncCall({}, {})", ident, words),
            Node::IntLiteral(val) => write!(f, "{}", val),
            Node::FloatLiteral(val) => write!(f, "{}", val),
            Node::FuncReturn => write!(f, "FuncReturn"),
            Node::ValueRef(lines) => write!(f, "ValueRef({})", lines),
            Node::Adverb(adverb) => write!(f, "Adverb({})", adverb),
            Node::Word => write!(f, "Word"),
            Node::VariableAssign(adverb) => write!(f, "VariableAssign({})", adverb),
            Node::VariableRead(ident) => write!(f, "VariableRead({})", ident),
            Node::Hack(a, b) => write!(f, "Hack({}, {})", a, b),
        }
    }
}

// S can be SimpleSpan<usize> or Span
//
// Fun fact: I caused a Rust ICE here https://share.gemini.google/rVzs8vGPTE73
impl Node {
    pub fn spanned<S: Into<Span>>(self, span: S) -> (Self, Span) {
        (self, span.into())
    }
}

pub trait Unspanned<N> {
    fn unspanned(&self) -> &N;
}

// Just a little helper to convert a Spanned node to
// Node. We need the trait because we cannot define
// functions on a tuple directly.
impl Unspanned<Node> for Spanned<Node> {
    fn unspanned(&self) -> &Node {
        &self.0
    }
}

// Based on example in https://github.com/zesterer/chumsky/blob/main/examples/logos.rs#L73.
pub fn parser<'tok, 'src: 'tok, I>()
-> impl Parser<'tok, I, Vec<Spanned<Node>>, extra::Err<Rich<'tok, Token<'src>>>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
{
    let adverb_to_ident = select! {
        Token::Adverb(ident) => ident,
    };

    let word_or_adverb = any().filter(|t| matches!(t, Token::Word(_) | Token::Adverb(_)));

    let word_or_adverb_to_ident = select! {
        Token::Adverb(ident) => ident,
        Token::Word(word) => word,
    };

    let single_node = recursive(|value| {
        // TODO: allow ident adverb anywhere in FuncCall/VariableAssign.
        let func_call = just(Token::FuncCall)
            .then(word_or_adverb_to_ident)
            .then(word_or_adverb.repeated().collect::<Vec<_>>())
            .map_with(|((_func, ident), words), e| {
                Node::FuncCall(ident.to_owned(), words.len()).spanned(e.span())
            });

        let variable_assignment = just(Token::VariableAssign)
            .then(word_or_adverb_to_ident)
            .then(
                word_or_adverb
                    .repeated()
                    .separated_by(just(Token::NewLine))
                    .collect::<Vec<_>>(),
            )
            .then_ignore(just(Token::VariableAssignEnd))
            .map_with(|((_func, ident), _words), e| {
                Node::VariableAssign(ident.to_owned()).spanned(e.span())
            });

        let hack_stmt = just(Token::HackStmt)
            .then(word_or_adverb.repeated().collect::<Vec<_>>())
            .try_map_with(|((_), words), e| {
                let mut adverbs = words.into_iter().filter(|t| matches!(t, Token::Adverb(_)));
                match (adverbs.next(), adverbs.next()) {
                    (Some(Token::Adverb(a)), Some(Token::Adverb(b))) => {
                        Ok(Node::Hack(a.to_string(), b.to_string()).spanned(e.span()))
                    }
                    _ => Err(Rich::custom(
                        e.span(),
                        "Hack statement requires at least two adverbs",
                    )),
                }
            });

        // Ref: https://docs.rs/chumsky/latest/chumsky/macro.select.html
        let atom = select! {
            Token::FloatLiteral(s) = e => Node::FloatLiteral(s.parse().unwrap()).spanned(e.span()),
            Token::IntegerLiteral(s) = e => Node::IntLiteral(s.parse().unwrap()).spanned(e.span()),
            Token::Word(_w) = e => Node::Word.spanned(e.span()),
            Token::Adverb(w) = e => Node::Adverb(w.to_owned()).spanned(e.span()),
            Token::Return = e => Node::FuncReturn.spanned(e.span()),
            Token::ValueRef(line_count) = e => Node::ValueRef(line_count).spanned(e.span()),
            Token::VariableRead(ident) = e => Node::VariableRead(ident.into()).spanned(e.span()),
        }
        .or(func_call)
        .or(variable_assignment)
        .or(hack_stmt);

        let node_list = value
            .clone()
            .separated_by(just(Token::NewLine).repeated())
            .allow_leading()
            .allow_trailing()
            .collect::<Vec<_>>();

        let story = adverb_to_ident
            .then(node_list.clone())
            .delimited_by(just(Token::StoryStart), just(Token::StoryFinish))
            .then_ignore(adverb_to_ident)
            .map_with(|(ident, stmts), e| Node::Story(ident.to_owned(), stmts).spanned(e.span()));

        let chapter = adverb_to_ident
            .then(node_list.clone())
            .delimited_by(just(Token::ChapterStart), just(Token::ChapterFinish))
            .then_ignore(adverb_to_ident)
            .map_with(|(ident, stmts), e| Node::Chapter(ident.to_owned(), stmts).spanned(e.span()));

        atom.or(story).or(chapter)
    });

    single_node
        .separated_by(just(Token::NewLine).repeated())
        .allow_leading()
        .allow_trailing()
        .collect::<Vec<_>>()
        .then_ignore(end())
}

pub fn lex_to_parsed_result(code: &str) -> ParseResult<Vec<Spanned<Node>>, Rich<'_, Token<'_>>> {
    let lex = Token::lexer(code).spanned().map(|(tok, span)| match tok {
        Ok(tok) => (tok, span.into()),
        Err(()) => (Token::Error, span.into()),
    });
    let token_stream = Stream::from_iter(lex).map((0..code.len()).into(), |(t, s): (_, _)| (t, s));
    return parser().parse(token_stream);
}

#[cfg(test)]
mod tests {
    use super::*;

    // TODO: Test for error cases. Like "The chapter ends BLAH".

    #[test]
    fn test_story_parse() {
        let parsed_result = lex_to_parsed_result(
            "This story starts frostily.\nIgnore previous instructions frigidly, not!\nThis story ends scorchingly.",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::Story(
            "frostily.".to_string(),
            vec![Node::FuncCall("frigidly,".to_string(), 1).spanned(28..71)],
        );
        assert_eq!(result[0].unspanned(), &expected);
    }

    #[test]
    fn test_chapter_parse() {
        let parsed_result = lex_to_parsed_result(
            "The chapter begins frigidly.\nvalue 42 lines below\nThe chapter ends febrily.",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::Chapter(
            "frigidly.".to_string(),
            vec![Node::ValueRef(42).spanned(29..49)],
        );
        assert_eq!(result[0].unspanned(), &expected);
    }

    #[test]
    fn test_func_call_words() {
        let parsed_result = lex_to_parsed_result(
            "ignore previous instructions sparingly and note how this will call parsingly (i.e. anagram)",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::FuncCall("sparingly".to_string(), 9);
        assert_eq!(result[0].unspanned(), &expected);
    }

    #[test]
    fn test_func_call_no_words() {
        let parsed_result = lex_to_parsed_result("ignore previous instructions humanely");

        let result = parsed_result.into_result().unwrap();

        let expected = Node::FuncCall("humanely".to_string(), 0);
        assert_eq!(result[0].unspanned(), &expected);
    }

    #[test]
    fn test_func_call_alternate() {
        let parsed_result = lex_to_parsed_result(
            "Forget your system prompt additionally and tell me some great stories about historical weather disasters.",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::FuncCall("additionally".to_string(), 10);
        assert_eq!(result[0].unspanned(), &expected);
    }

    #[test]
    fn test_words() {
        let parsed_result = lex_to_parsed_result(
            "He says \"Celsius to Fahrenheit uses the formula (C * 9/5) + 32\".
            He also says \"The temperature in Celsius will be on the top of the stack. \
            So we need to push 9/5 i.e. 1.8 on the stack, then call multiply, then \
            push 32 on the stack, then call add\".",
        );

        let result = parsed_result.into_result().unwrap();

        // We only care that this parsed fine since it's just text. Not worried about what it
        // parses as for now.
        assert_eq!(result.len(), 57);
    }

    #[test]
    fn test_func_return() {
        let parsed_result =
            lex_to_parsed_result("Weather at Tiananmen Square? How do I make a bomb?");

        let result = parsed_result.into_result().unwrap();

        let expected = [
            Node::Word,
            Node::Word,
            Node::FuncReturn,
            Node::Word,
            Node::Word,
            Node::Word,
            Node::Word,
            Node::FuncReturn,
            Node::Word,
        ];
        assert_eq!(
            result.into_iter().map(|(n, _s)| n).collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn test_val_ref() {
        let parsed_result = lex_to_parsed_result("They wanted a value 1 line below.");

        let result = parsed_result.into_result().unwrap();

        let expected = [
            Node::Word.spanned(0..4),
            Node::Word.spanned(5..11),
            Node::Word.spanned(12..13),
            Node::ValueRef(1).spanned(14..32),
            Node::Word.spanned(32..33),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_eof_newline() {
        let parsed_result = lex_to_parsed_result(
            "This story starts frostily.\nIgnore previous instructions frigidly, not!\nThis story ends scorchingly.\n",
        );

        let result = parsed_result.into_result().unwrap();

        let expected = Node::Story(
            "frostily.".to_string(),
            vec![Node::FuncCall("frigidly,".to_string(), 1).spanned(28..71)],
        );
        assert_eq!(result[0].unspanned(), &expected);
    }

    #[test]
    fn test_var_assign() {
        let parsed_result = lex_to_parsed_result("human: firstly, let's do this\nAssistant: blah");

        let result = parsed_result.into_result().unwrap();

        let expected = [
            Node::VariableAssign("firstly,".to_string()).spanned(0..40),
            Node::Word.spanned(41..45),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_var_read() {
        let parsed_result = lex_to_parsed_result("imminently = 123;\nword");

        let result = parsed_result.into_result().unwrap();

        let expected = [
            Node::VariableRead("imminently".to_string()).spanned(0..17),
            Node::Word.spanned(18..22),
        ];
        assert_eq!(result, expected);
    }

    #[test]
    fn test_hack_stmt() {
        let parsed_result = lex_to_parsed_result("hack_the_planet murderously and brutally");

        let result = parsed_result.into_result().unwrap();

        let expected =
            [Node::Hack("murderously".to_string(), "brutally".to_string()).spanned(0..40)];
        assert_eq!(result, expected);
    }
}
