use std::fmt::{Display, Formatter};
use std::rc::Rc;
use crate::commands::{Command, Environment};

#[derive(Default)]
pub struct Line {
    pub file: String,
    pub line_number: usize,
    pub contents: String,
}


#[derive(Clone, Debug, Default, PartialEq)]
pub struct Location {
    pub file: String,
    pub line_number: usize,
    pub column: usize,
}

impl Location {
    pub fn rc_from_line_and_column(line: &Line, column: usize) -> Rc<Location> {
        Rc::new(Location::from_line_and_column(line, column))
    }

    pub fn from_line_and_column(line: &Line, column: usize) -> Location {
        Location {
            file: line.file.to_string(),
            line_number: line.line_number,
            column,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupType {
    Brace,
    Environment(Rc<Environment>),
    RequiredArgument,
    OptionalArgument,
    ArbitraryDelim(String), // must be string so we can write, e.g., \verb🇨🇦something🇨🇦
}

#[derive(Debug, PartialEq, Default)]
pub struct ErrorContext {
    pub location: Location,
    pub line_contents: String,
}

impl ErrorContext {
    pub fn from_line_and_column(line: &Line, column: usize) -> ErrorContext {
        ErrorContext {
            location: Location::from_line_and_column(line, column),
            line_contents: line.contents.clone()
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum FinlError {
    UndefinedCommand(ErrorContext, String),
    Unimplemented(ErrorContext),
    BlankLineWhileParsingCommandArguments(ErrorContext, String, usize), // .2 is the argument number
    UnexpectedEOFWhileParsingCommandArguments(ErrorContext, String, usize),
    UnexpectedCloseBrace(ErrorContext, Option<GroupType>),
}

impl Display for FinlError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // todo
        write!(f, "todo")
    }
}

#[derive(Debug, PartialEq)]
pub enum Token {
    ParsedText(Location, String),
    Math(Location, String),
    Command(Location, Rc<Command>, Vec<Token>),
    Environment(Location, Rc<Environment>, Vec<Token>, Vec<Token>),
    RawText(Location, String),
    Bgroup(Location),
    Egroup(Location),
    Tokens(Location, Vec<Token>) // Q: Does this make sense? Yes, for arguments to commands.
}

impl Display for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::ParsedText(_, text) => write!(f, "{}", text),
            Token::Math(_, math) => write!(f, "{}", math),
            // Todo: allow outputting the arguments
            Token::Command(_, cmd, _args) => write!(f, "\\{}", cmd.name),
            // TODO: allow outputting arguments and body
            Token::Environment(_, env, _args, _body) => write!(f, "\\begin{{{}}}…\\end{{{}}}", env.name, env.name),
            Token::RawText(_, text) => write!(f, "{}", text),
            Token::Tokens(_, tokens) => {
                write!(f, "[[")?;
                for token in tokens {
                    write!(f, "{}", token)?;
                }
                write!(f, "]]")
            },
            Token::Bgroup(_) => write!(f, "bgroup"),
            Token::Egroup(_) => write!(f, "egroup"),
        }
    }
}

