use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::commands::{Command, Environment};

#[derive(Debug, PartialEq)]
pub enum Token {
    ParsedText(String),
    Math(String),
    Command(Rc<Command>, Vec<Token>),
    Environment(Rc<Environment>, Vec<Token>, Vec<Token>),
    RawText(String),
    Bgroup,
    Egroup,
    Tokens(Vec<Token>) // Q: Does this make sense? Yes, for arguments to commands.
}

impl Display for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::ParsedText(text) => write!(f, "{}", text),
            Token::Math(math) => write!(f, "{}", math),
            // Todo: allow outputting the arguments
            Token::Command(cmd, _args) => write!(f, "\\{}", cmd.name),
            // TODO: allow outputting arguments and body
            Token::Environment(env, _args, _body) => write!(f, "\\begin{{{}}}…\\end{{{}}}", env.name, env.name),
            Token::RawText(text) => write!(f, "{}", text),
            Token::Tokens(tokens) => {
                write!(f, "[[");
                for token in tokens {
                    write!(f, "{}", token);
                }
                write!(f, "]]")
            },
            Token::Bgroup => write!(f, "bgroup"),
            Token::Egroup => write!(f, "egroup"),
        }
    }
}

