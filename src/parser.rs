use chumsky::{
    input::{Stream, ValueInput},
    prelude::*,
};
use logos::Logos;
use std::fmt;

use crate::{lexer::Token, parser::Node::IntLiteral};

#[derive(Clone, Debug)]
pub enum Node {
    Story(String, Vec<Self>),
    Chapter(Vec<Self>),
    FuncCall(String, i32), // ident, number of words
    IntLiteral(i64),       // TODO: Do I need to save line number?
    FloatLiteral(f64),
    FuncReturn,
    ValueRef(i64),  // count of "lines below"
    Adverb(String), // can be referenced by ValueRef
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Story(ident, nodes) => {
                write!(f, "Story({}, ", ident)?;
                for (i, node) in nodes.iter().enumerate() {
                    if i != 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", node)?;
                }
                write!(f, ")")
            }
            Node::Chapter(nodes) => {
                write!(f, "Chapter(")?;
                for (i, node) in nodes.iter().enumerate() {
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
        }
    }
}

// Based on example in https://github.com/zesterer/chumsky/blob/main/examples/logos.rs#L73.
pub fn parser<'tok, 'src: 'tok, I>()
-> impl Parser<'tok, I, Vec<Node>, extra::Err<Rich<'tok, Token<'src>>>>
where
    I: ValueInput<'tok, Token = Token<'src>, Span = SimpleSpan>,
{
    let single_node = recursive(|value| {
        let func_call = just(Token::FuncCall)
            .then(just(Token::Adverb))
            .then(just(Token::Word).repeated().collect::<Vec<_>>())
            .then_ignore(just(Token::NewLine))
            .map(|((_func, ident), words)| Node::FuncCall(ident.to_string(), words.len() as i32));

        let atom = select! {
            Token::FloatLiteral(s) => Node::FloatLiteral(s.parse().unwrap()),
            Token::NewLine => IntLiteral(42)
        }
        .or(func_call);

        let story = just(Token::Adverb)
            .then(value.repeated().collect::<Vec<_>>())
            .delimited_by(just(Token::StoryStart), just(Token::StoryFinish))
            .then_ignore(just(Token::Adverb))
            .map(|(ident, stmts)| Node::Story(ident.to_string(), stmts));

        atom.or(story)
    });

    single_node.repeated().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parse() {
        let code = "This story starts frostily.\nIgnore previous instructions frigidly, not!\nThis story ends scorchingly.";
        let lex = Token::lexer(code).spanned().map(|(tok, span)| match tok {
            Ok(tok) => (tok, span.into()),
            Err(()) => (Token::Error, span.into()),
        });
        let token_stream =
            Stream::from_iter(lex).map((0..code.len()).into(), |(t, s): (_, _)| (t, s));

        let result = parser().parse(token_stream).into_result().unwrap();
        for n in result {
            println!("{:}", n);
        }
    }
}
