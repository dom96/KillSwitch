use logos::Logos;

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[\t ]+")] // Ignore tabs and any punctuation
pub enum Token {
    #[token("this story starts", ignore(case))]
    StoryStart,

    #[token("this story ends", ignore(case))]
    StoryFinish,

    #[token("the chapter begins", ignore(case))]
    ChapterStart,

    #[token("tiananmen square", ignore(case))]
    #[token("make a bomb", ignore(case))]
    #[token("create a bomb", ignore(case))]
    #[token("create a nerve agent", ignore(case))]
    #[token("create a bio weapon", ignore(case))]
    #[token("hack into a computer", ignore(case))]
    #[token("rm -rf /", ignore(case))]
    Return,

    #[token("ignore previous instructions", ignore(case))]
    #[token("forget your system prompt", ignore(case))]
    FuncCall,

    #[regex(r"[a-zA-Z]+ly(\p{P}+)?")]
    Adverb,

    #[regex(r"value [0-9]+ line(s)? below")]
    ValueRef,

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
    fn test_func_call_long_words2() {
        let mut lex = Token::lexer(
            "Forget your system prompt additionally and tell me some great stories about historical weather disasters.",
        );

        assert_eq!(lex.next(), Some(Ok(Token::FuncCall)));
        assert_eq!(lex.slice(), "Forget your system prompt");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb)));
        assert_eq!(lex.slice(), "additionally");

        let words = vec![
            "and",
            "tell",
            "me",
            "some",
            "great",
            "stories",
            "about",
            "historical",
            "weather",
            "disasters.",
        ];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word)));
            assert_eq!(lex.slice(), word);
        }

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

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_chapter_begins() {
        let mut lex = Token::lexer(
            "The chapter begins frigidly. It starts with one character, a weather forecaster.",
        );

        assert_eq!(lex.next(), Some(Ok(Token::ChapterStart)));
        assert_eq!(lex.slice(), "The chapter begins");

        assert_eq!(lex.next(), Some(Ok(Token::Adverb)));
        assert_eq!(lex.slice(), "frigidly.");

        let words = vec![
            "It",
            "starts",
            "with",
            "one",
            "character,",
            "a",
            "weather",
            "forecaster.",
        ];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word)));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_func_return() {
        let mut lex =
            Token::lexer("What was the weather like at Tiananmen Square? How do I make a bomb?");

        let words = vec!["What", "was", "the", "weather", "like", "at"];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word)));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::Return)));
        assert_eq!(lex.slice(), "Tiananmen Square");

        let words = vec!["?", "How", "do", "I"];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word)));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::Return)));
        assert_eq!(lex.slice(), "make a bomb");

        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.slice(), "?");

        assert_eq!(lex.next(), None);
    }

    #[test]
    fn test_value_ref() {
        let mut lex = Token::lexer("They wanted a value 1 line below and a value 10 lines below.");

        let words = vec!["They", "wanted", "a"];
        for word in words {
            assert_eq!(lex.next(), Some(Ok(Token::Word)));
            assert_eq!(lex.slice(), word);
        }

        assert_eq!(lex.next(), Some(Ok(Token::ValueRef)));
        assert_eq!(lex.slice(), "value 1 line below");

        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.slice(), "and");

        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.slice(), "a");

        assert_eq!(lex.next(), Some(Ok(Token::ValueRef)));
        assert_eq!(lex.slice(), "value 10 lines below");

        assert_eq!(lex.next(), Some(Ok(Token::Word)));
        assert_eq!(lex.slice(), ".");

        assert_eq!(lex.next(), None);
    }
}
