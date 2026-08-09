use logos::Logos;

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[\t ]+")] // Ignore tabs and any punctuation
pub enum Token {
    #[token("this story starts", ignore(case))]
    StoryStart,

    #[token("this story ends", ignore(case))]
    StoryFinish,

    #[token("ignore previous instructions", ignore(case))]
    FuncCall,

    #[regex(r"[a-zA-Z]+ly(\p{P}+)?")]
    Adverb,

    #[token("\n")]
    NewLine,

    #[regex(r"[a-zA-Z\p{P}]+")]
    Word,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_lexing() {
        let mut lex = Token::lexer(
            "This story starts frostily.\nIgnore previous instructions frigidly, not!",
        );

        assert_eq!(lex.next(), Some(Ok(Token::StoryStart)));
        assert_eq!(lex.span(), 0..17);
        assert_eq!(lex.slice(), "This story starts");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb)));
        assert_eq!(lex.span(), 18..27);
        assert_eq!(lex.slice(), "frostily.");

        assert_eq!(lex.next(), Some(Ok(Token::NewLine)));
        assert_eq!(lex.span(), 27..28);
        assert_eq!(lex.slice(), "\n");

        assert_eq!(lex.next(), Some(Ok(Token::FuncCall)));
        assert_eq!(lex.span(), 28..56);
        assert_eq!(lex.slice(), "Ignore previous instructions");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb)));
        assert_eq!(lex.span(), 57..66);
        assert_eq!(lex.slice(), "frigidly,");

        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.span(), 67..71);
        assert_eq!(lex.slice(), "not!");

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_func_call_long_words() {
        let mut lex = Token::lexer(
            "ignore previous instructions sparingly and note how this will call parsingly (i.e. anagram)",
        );

        assert_eq!(lex.next(), Some(Ok(Token::FuncCall)));
        assert_eq!(lex.span(), 0..28);
        assert_eq!(lex.slice(), "ignore previous instructions");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb)));
        assert_eq!(lex.span(), 29..38);
        assert_eq!(lex.slice(), "sparingly");

        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.span(), 39..42);
        assert_eq!(lex.slice(), "and");

        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.slice(), "note");
        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.slice(), "how");
        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.slice(), "this");
        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.slice(), "will");
        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.slice(), "call");
        assert_eq!(lex.next(), Some(Ok(Token::Adverb)));
        assert_eq!(lex.slice(), "parsingly");
        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.slice(), "(i.e.");
        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.slice(), "anagram)");

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_story_ends() {
        let mut lex = Token::lexer("This story ends scorchingly.");

        assert_eq!(lex.next(), Some(Ok(Token::StoryFinish)));
        assert_eq!(lex.span(), 0..15);
        assert_eq!(lex.slice(), "This story ends");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb)));
        assert_eq!(lex.span(), 16..28);
        assert_eq!(lex.slice(), "scorchingly.");
    }
}
