use crate::commands::{Command, Environment};
use crate::tokens::{Argument, ArgumentType, Token};

use std::collections::HashMap;

use unicode_categories::UnicodeCategories;
use std::rc::Rc;

mod tokens;
mod commands;

pub struct Line {
    pub file: String,
    pub line_number: usize,
    pub contents: String
}

#[derive(Clone)]
pub struct Location {
    pub file: String,
    pub line_number: usize,
    pub column: usize
}

impl Location {
    pub fn rc_from_line_and_column(line: &Line, column: usize) -> Rc<Location> {
        Rc::new(Location::from_line_and_column(line, column))
    }

    pub fn from_line_and_column(line: &Line, column: usize) -> Location {
        Location {
            file: line.file.to_string(),
            line_number: line.line_number,
            column
        }
    }
}

enum ParserState {
    StartingCommand(usize),
    NamedCommand(String, Rc<Location>),
    SymbolCommand(String, Rc<Location>),
    ParsingText(usize),
    ParsingComment(usize),
    IgnoringSpacesAfterNamedCommand(Rc<Command>, Rc<Location>),
    // We've identified a command, we're now looking for its arguments
    // .0 has the command whose arguments we're parsing,
    // .1 is the index into command.parameters
    // .2 is the Location of the beginning of the command
    ParsingArguments(Rc<Command>, usize, Rc<Location>),
}

pub struct Parser {
    commands: HashMap<String, Rc<Command>>,
    environments: HashMap<String, Rc<Environment>>,
    parser_state: ParserState
}

#[derive(Debug, PartialEq)]
pub enum FinlError {
    UndefinedCommand(String)
}

impl Parser {
    pub fn default() -> Parser {
        Parser {
            commands: HashMap::default(),
            environments: HashMap::default(),
            parser_state: ParserState::ParsingText(0),
        }
    }

    pub fn define_command(&mut self, name: &str, args: Vec<(Argument, ArgumentType)>) {
        self.commands.insert(name.to_string(), Rc::new(Command::new(name, args)));
    }

    pub fn parse(&mut self, line: Line) -> (Vec<(Token, Rc<Location>)>, Vec<(FinlError, Rc<Location>)>) {
        let mut tokens = Vec::default();
        let mut errors = Vec::default();
        'parsing: for (column, ch) in line.contents.char_indices() {
            'reparse: loop {
                match &mut self.parser_state {
                    ParserState::ParsingText(start) => {
                        match ch {
                            '\\' => {
                                if *start != column {
                                    tokens.push((Token::ParsedText(line.contents.get(*start..column).unwrap().to_string()), Location::rc_from_line_and_column(&line, column)));
                                }
                                self.parser_state = ParserState::StartingCommand(column);
                            },
                            _ => {}
                        }
                    }
                    ParserState::StartingCommand(start_column) => {
                        if Parser::letter_test(ch) {
                            self.parser_state = ParserState::NamedCommand(String::from(ch), Location::rc_from_line_and_column(&line, *start_column));
                        }
                        else {
                            self.parser_state = ParserState::SymbolCommand(String::from(ch), Location::rc_from_line_and_column(&line, *start_column));
                        }
                    }
                    ParserState::NamedCommand(command_name, loc) => {
                        if Parser::letter_test(ch) {
                            command_name.push(ch)
                        }
                        else {
                            let command = self.commands.get(command_name);
                            match command {
                                // Undefined comand
                                None => {
                                    errors.push((FinlError::UndefinedCommand(command_name.to_string()), loc.clone()));
                                }
                                Some(command) => {
                                    if ch.is_whitespace() {
                                        self.parser_state = ParserState::IgnoringSpacesAfterNamedCommand(command.clone(), loc.clone());
                                        continue 'parsing;
                                    }
                                }
                            }
                        }
                    }
                    ParserState::IgnoringSpacesAfterNamedCommand(command, loc) => {
                        if !ch.is_whitespace() {
                            if command.parameters.is_empty() {
                                tokens.push((Token::Command(command.clone(), Default::default()), loc.clone()));
                                self.parser_state = ParserState::ParsingText(0);
                                continue 'reparse;
                            }
                            else {
                                ParserState::ParsingArguments(command.clone(), 0, loc.clone());
                            }
                        }
                    }
                    ParserState::SymbolCommand(_, _) => {}
                    ParserState::ParsingComment(_) => {}
                    ParserState::ParsingArguments(command, index, location) => {}
                }
                continue 'parsing;
            }
        }
        (tokens, errors)
    }

    pub fn flush(&mut self) -> (Vec<(Token, Location)>, Vec<FinlError>) {
        (Vec::new(), Vec::new())
    }

    fn letter_test(ch: char) -> bool {
        ch.is_letter() || ch.is_mark_nonspacing() || ch.is_mark_spacing_combining()
    }


}



#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn undefined_commands_are_flagged_correctly() {
        let mut parser = Parser::default();
        let (tokens, errors) = parser.parse(
            Line {
                file: "myfile".to_string(),
                line_number: 1,
                contents: " \\undefined ".to_string()
            }
        );
        assert_eq!(tokens.len(), 1);
        assert_eq!(errors.len(), 1);
        assert_eq!(tokens.get(0).unwrap().0, Token::ParsedText(" ".to_string()));
        assert_eq!(errors.get(0).unwrap().0, FinlError::UndefinedCommand("undefined".to_string()));
        let loc = &errors.get(0).unwrap().1;
        assert_eq!(loc.line_number, 1);
        assert_eq!(loc.file, "myfile".to_string());
        assert_eq!(loc.column,1);
    }

    #[test]
    fn can_parse_a_command_with_no_arguments() {
        let mut parser = Parser::default();
        parser.define_command("foo", Vec::default());
        let (tokens, errors) = parser.parse(Line {
            file: "".to_string(),
            line_number: 1,
            contents: "\\foo a".to_string() // Need to have a text token here to flush the command.
        });
        assert_eq!(errors.len(), 0);
        assert!(tokens.len() > 0);
        let parsed_command = &tokens[0].0;
        match parsed_command {
            Token::Command(cmd, args) => {
                assert_eq!(cmd.name, "foo".to_string());
                assert_eq!(args.len(), 0);
            }
            token => panic!("First token was not \\foo but was {:?}", token)
        }
    }
}