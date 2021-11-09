use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::commands::{Command, Environment};

#[derive(PartialEq, Debug)]
pub enum Token {
    ParsedText(String),
    Math(String),
    Command(Rc<Command>, Vec<Token>),
    Environment(Rc<Environment>, Vec<Token>, Vec<Token>),
    RawText(String),
    Tokens(Vec<Token>)
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
                write!(f, "[[token list]]")
            },
        }
    }
}

