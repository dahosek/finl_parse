use std::collections::HashMap;
use std::iter::Peekable;
use std::rc::Rc;
use std::str::CharIndices;

use unicode_categories::UnicodeCategories;

use crate::commands::{Command, Environment, ParameterFormat, ParameterType};
use crate::tokens::Token;
use std::mem;

mod tokens;
mod commands;

#[derive(Default)]
pub struct Line {
    pub file: String,
    pub line_number: usize,
    pub contents: String,
}

#[derive(Clone, Debug)]
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

#[derive(Debug, PartialEq)]
pub enum FinlError {
    UndefinedCommand(String),
    Unimplemented,
    BlankLineWhileParsingCommandArguments(String, usize), // .2 is the argument number
    UnexpectedEOFWhileParsingCommandArguments(String, usize),
    UnexpectedCloseBrace(Option<GroupType>),
}


enum CommandContext {
    Text,
    UserCommandDefinition,
    Math,
}

#[derive(PartialEq,Debug)]
enum SkipWhiteSpaceOutcome {
    Skipped,
    FoundBlankLine,
    EndOfFile
}

#[derive(Clone, Debug, PartialEq)]
pub enum GroupType {
    Brace,
    Environment(Rc<Environment>)
}

struct ParserContext<'a> {
    commands: HashMap<String, Rc<Command>>,
    environments: HashMap<String, Rc<Environment>>,
    lines: Box<dyn Iterator<Item=&'a str> + 'a>,
    line: Line,
    char_iterator: Peekable<CharIndices<'a>>,
    output: Vec<(Result<Token, FinlError>, Location)>,
    stack: Vec<GroupType>,
}

impl<'a> ParserContext<'a> {
    fn push_eol_text_block(&mut self, start: usize) {
        self.push_text_block(start, self.line.contents.len());
    }
    fn push_text_block(&mut self, start: usize, end: usize) {
        if start != end {
            self.push_token(Token::ParsedText(self.line.contents.get(start..end).unwrap().to_string()), start)
        }
    }
    fn push_error(&mut self, error: FinlError, column: usize) {
        self.output.push((Err(error), Location::from_line_and_column(&self.line, column)));
    }

    fn push_command(&mut self, command: Rc<Command>, args: Vec<Token>, column: usize) {
        self.push_token(Token::Command(command, args), column)
    }

    fn push_token(&mut self, token: Token, column: usize) {
        self.output.push((
            Ok(token),
            Location::from_line_and_column(&self.line, column)
            ))
    }

    fn from_string(input: &'a str) -> ParserContext<'a> {
        let mut context :ParserContext<'a> = ParserContext {
            commands: Default::default(),
            environments: Default::default(),
            lines: Box::new(input.lines()),
            line: Line {
                file: "STRING CONSTANT".to_string(),
                line_number: 0,
                contents: Default::default()
            },
            char_iterator: "".char_indices().peekable(),
            output: vec![],
            stack: vec![]
        };
        context.next_line();
        context
    }

    fn skip_whitespace(&mut self) -> SkipWhiteSpaceOutcome {
        let mut found_blank_line = false;
        let line_no = self.line.line_number;
        let start_column = if let Some((start_column, _)) = self.char_iterator.peek() {
            *start_column
        }
        else {
            self.next_line();
            if self.line.line_number == 0 {
                return SkipWhiteSpaceOutcome::EndOfFile;
            }
            0 as usize
        };
        while let Some((_, ch)) = self.char_iterator.peek() {
            if ch.is_whitespace() {
                self.char_iterator.next();
            }
            else {
                break;
            }
        }
        if self.char_iterator.peek().is_none() {
            self.next_line();
            if self.line.line_number == 0 {
                return SkipWhiteSpaceOutcome::EndOfFile;
            }
            // See if there's a non-blank line
        }
        if found_blank_line {
            SkipWhiteSpaceOutcome::FoundBlankLine
        }
        else {
            SkipWhiteSpaceOutcome::Skipped
        }
    }

    fn next_line(&mut self) {
        match self.lines.next() {
            None => {
                self.line = Line::default();
                self.char_iterator = "".char_indices().peekable();
            }
            Some(line) => {
                self.line.line_number += 1;
                self.line.contents = String::from(line);
                self.char_iterator = line.char_indices().peekable();
            }
        }
    }

}

impl<'a> Default for ParserContext<'a> {
    fn default() -> Self {
        ParserContext {
            commands: Default::default(),
            environments: Default::default(),
            lines: Box::new("".lines()),
            line: Default::default(),
            char_iterator: "".char_indices().peekable(),
            output: vec![],
            stack: vec![]
        }
    }
}

fn text_parse(context: &mut ParserContext) {
    if let Some((0, _)) = context.char_iterator.peek().clone() {
        // Skip leading whitespace at beginnings of lines
        //context.char_iterator.skip_while(|(_, ch)| ch.is_whitespace());
        context.skip_whitespace();
    }
    let mut start;
    match context.char_iterator.peek() {
        None => {
            return;
        }
        Some((column, _)) => {
            start = *column;
        }
    }
    while let Some((column, ch)) = context.char_iterator.peek().cloned() {
        match ch {
            '\\' => {
                context.push_text_block(start, column);
                command_parse(context, CommandContext::Text);
                start = get_column(context);
            }
            // If we have a `%`, we dump whatever's left and finish the line.
            '%' => {
                context.push_text_block(start, column);
                context.next_line();
                return ;
            }
            '{' => {
                context.push_text_block(start, column);
                context.push_token(Token::Bgroup, column);
                context.char_iterator.next();
                context.stack.push(GroupType::Brace);
                start = get_column(context);
            }
            '}' => {
                context.push_text_block(start, column);
                let top_of_stack = context.stack.last().cloned();
                if let Some(GroupType::Brace) = context.stack.pop() {
                    context.push_token(Token::Egroup, column);
                }
                else {
                    context.push_error(FinlError::UnexpectedCloseBrace(top_of_stack.clone()), column);
                    if let Some(group_type) = top_of_stack {
                        context.stack.push(group_type);
                    }
                }
                context.char_iterator.next();
                start = get_column(context);
            }
            _ => {
                context.char_iterator.next();
            }
        }
    }
    context.push_eol_text_block(start);
}

fn get_column(context: &mut ParserContext) -> usize {
    match context.char_iterator.peek() {
        None => {
            context.next_line();
            0
        }
        Some((column, _)) => {
            *column
        }
    }}

fn command_parse(mut context: &mut ParserContext, command_context: CommandContext)  {
    let (command_start, _) = context.char_iterator.next().expect("This should not happen"); // get column of backslash
    let command_name = get_command_name(&mut context, &command_context);
    match context.commands.get(&command_name).cloned() {
        //map(|r| r.clone()) {
        None => {
            context.push_error(FinlError::UndefinedCommand(command_name), command_start);
        }
        Some(command) => {
            let mut args = Vec::with_capacity(command.parameters.len());
            let mut parameter_number = 0;
            for (format, ptype) in &command.parameters {
                parameter_number += 1;
                let possible_arg = match format {
                    ParameterFormat::Star => { Err(FinlError::Unimplemented) }
                    ParameterFormat::Required => {
                        parse_required_argument(context, &command.name, parameter_number, &command_context, *ptype)
                    }
                    ParameterFormat::RequiredWithBraces => { Err(FinlError::Unimplemented) }
                    ParameterFormat::Optional => { Err(FinlError::Unimplemented) }
                    ParameterFormat::ArbitraryDelimiters => { Err(FinlError::Unimplemented) }
                };
                match possible_arg {
                    Ok(token) => {
                        args.push(token);
                    }
                    Err(err) => {
                        context.push_error(err, command_start);
                        return;
                    }
                }
            }
            context.push_command(command.clone(), args, command_start);
        }
    }
}

fn get_command_name(context: &mut ParserContext, command_context: &CommandContext) -> String {
    let name_start = context.char_iterator.peek();
    match name_start {
        None => " ".to_string(), // backslash at end of line ≡ \␣
        Some((_, ch)) => {
            if letter_test(*ch) {
                let mut command_name = String::new();
                // consume letters:
                loop {
                    if let Some((_, ch)) = context.char_iterator.peek() {
                        if letter_test(*ch) {
                            command_name.push(*ch);
                            context.char_iterator.next();
                        }
                        else {
                            break;
                        }
                    }
                    else {
                        // We reached the end of the line
                        break;
                    }
                }

                // Skip any trailing whitespace:
                while let Some((_, ch)) = context.char_iterator.peek() {
                    if ch.is_whitespace() {
                        context.char_iterator.next();
                    }
                    else {
                        break;
                    }
                }
                command_name
            }
            else {
                // TODO: This doesn't correctly handle characters like 🇨🇦 or 🐻‍❄️
                ch.to_string()
            }
        }
    }
}

fn parse_required_argument(mut context: &mut ParserContext, command: &String, parameter_number: usize, command_context: &CommandContext, ptype: ParameterType) -> Result<Token,FinlError> {
    match context.skip_whitespace() {
        SkipWhiteSpaceOutcome::Skipped => {}
        SkipWhiteSpaceOutcome::FoundBlankLine => {
            return Err(FinlError::BlankLineWhileParsingCommandArguments(command.clone(), parameter_number));
        }
        SkipWhiteSpaceOutcome::EndOfFile => {
            return Err(FinlError::UnexpectedEOFWhileParsingCommandArguments(command.clone(), parameter_number));
        }
    }

    // Check for open brace
    // If open brace parse to close brace
    // Otherwise grab next token.
    Err(FinlError::Unimplemented)
}

fn letter_test(ch: char) -> bool {
    ch.is_letter() || ch.is_mark_nonspacing() || ch.is_mark_spacing_combining()
}

pub struct Parser<'a> {
    context: ParserContext<'a>
}


impl<'a> Parser<'a> {
    pub fn from_string(input: &'a str) -> Parser<'a> {
        Parser {
            context: ParserContext::from_string(input)
        }
    }

    pub fn define_command(&mut self, name: &str, args: Vec<(ParameterFormat, ParameterType)>) {
        self.context.commands.insert(name.to_string(), Rc::new(Command::new(name, args)));
    }

    pub fn parse(&mut self) -> Vec<(Result<Token, FinlError>, Location)> {
        text_parse( &mut self.context);
        mem::take(&mut self.context.output)
    }
}


#[cfg(test)]
mod test {
    use super::*;
    use crate::tokens::Token::{Bgroup, Egroup};

    #[test]
    fn undefined_commands_are_flagged_correctly() {
        let mut parser = Parser::from_string("\\undefined");
        let mut output = parser.parse();
        assert_eq!(output.len(), 1);
        let (first_item, loc) = output.remove(0);
        assert!(first_item.is_err());
//        assert_eq!(tokens.get(0).unwrap().0, Token::ParsedText(" ".to_string()));
        let error = first_item.expect_err("First item should be an error");
        assert_eq!(error, FinlError::UndefinedCommand("undefined".to_string()));
        assert_eq!(loc.line_number, 1);
        assert_eq!(loc.file, "STRING CONSTANT".to_string());
        assert_eq!(loc.column, 0);
    }

    #[test]
    fn can_parse_a_command_with_no_arguments() {
        let mut parser = Parser::from_string("\\foo a");
        parser.define_command("foo", Vec::default());
        let mut output = parser.parse();
        assert_eq!(output.len(), 2);
        let (item1, loc1) = output.remove(0);
        let (item2, loc2) = output.remove(0);
        let command = item1.expect("First token should not be an error");
        if let Token::Command(command, args) = command {
            assert_eq!(command.name, "foo".to_string());
            assert_eq!(args.len(), 0);
        }
        else {
            panic!("First token was not a command but was: {}", command);
        }
        let text = item2.expect("Second token should not be an error");
        if let Token::ParsedText(text) = text {
            assert_eq!(text, "a".to_string());
        }
        else {
            panic!("Second token was not text but was: {}", text);
        }

    }

    #[test]
    fn braces_must_match() {
        let mut parser = Parser::from_string("{}");
        let mut output = parser.parse();
        // println!("{:?}", output);
        assert_eq!(output.len(), 2);
        let (open, _) = output.remove(0);
        assert_eq!(open.unwrap(), Bgroup);
        let (close, _) = output.remove(0);
        assert_eq!(close.unwrap(), Egroup);

        let mut parser = Parser::from_string("{n}");
        let mut output = parser.parse();
        // println!("{:?}", output);
        assert_eq!(output.len(), 3);
        let (open, _) = output.remove(0);
        assert_eq!(open.unwrap(), Bgroup);
        let (close, _) = output.remove(1);
        assert_eq!(close.unwrap(), Egroup);

        let mut parser = Parser::from_string("{\n}");
        let mut output = parser.parse();
        // println!("{:?}", output);
        assert_eq!(output.len(), 2);
        let (open, _) = output.remove(0);
        assert_eq!(open.unwrap(), Bgroup);
        let (close, _) = output.remove(0);
        assert_eq!(close.unwrap(), Egroup);

        let mut parser = Parser::from_string("{}}");
        let mut output = parser.parse();
        // println!("{:?}", output);
        assert_eq!(output.len(), 3);
        let (open, _) = output.remove(0);
        assert_eq!(open.unwrap(), Bgroup);
        let (close, _) = output.remove(0);
        assert_eq!(close.unwrap(), Egroup);
        let (err, _) = output.remove(0);
        assert_eq!(err.unwrap_err(), FinlError::UnexpectedCloseBrace(None));
    }

    #[test]
    fn skip_white_space_finds_blank_line() {
        let mut context = ParserContext::from_string("\n\na");
        assert_eq!(context.skip_whitespace(), SkipWhiteSpaceOutcome::FoundBlankLine);
    }

    #[test]
    fn blank_line_before_argument_is_error() {
        let mut parser = Parser::from_string("\\foo\n\n{a}");
        parser.define_command("foo", vec![(ParameterFormat::Required, ParameterType::ParsedTokens)]);
        let mut output = parser.parse();
        println!("{:?}", output);
        assert_eq!(output.len(), 1);
        let (item, loc) = output.remove(0);
        if let FinlError::BlankLineWhileParsingCommandArguments(command_name, parameter_number) = item.unwrap_err() {
            assert_eq!("foo", command_name);
            assert_eq!(1, parameter_number);
        }
        else {
            panic!("I thought this would be an error");
        }
    }

    #[test]
    fn can_parse_a_command_with_single_required_argument_parsed_text() {
        let mut parser = Parser::from_string("\\foo{a} \\foo b \\foo\\foo{c}");
        parser.define_command("foo", vec![(ParameterFormat::Required, ParameterType::ParsedTokens)]);
        let mut output = parser.parse();
        println!("{:?}", output);
        assert_eq!(output.len(), 3);
        let (item1, loc1) = output.remove(0);
        let (item2, loc2) = output.remove(0);
        let (item3, loc3) = output.remove(0);
        if let Token::Command(command, args) = item1.unwrap() {
            // first command
            assert_eq!(command.name, "foo".to_string());
            assert_eq!(args.len(), 1);
            if let Token::ParsedText(text) = &args[0] {
                assert_eq!("a", text);
            }
            else {
                panic!("First argument to {} was {}", command.name, args[0]);
            }
        }
        else {
            panic!("Expected \\foo");
        }

        if let Token::Command(command, args) = item2.unwrap() {
            // second command
            assert_eq!(command.name, "foo".to_string());
            assert_eq!(args.len(), 1);
            if let Token::ParsedText(text) = &args[0] {
                assert_eq!("b", text);
            }
            else {
                panic!("First argument to {} was {}", command.name, args[0]);
            }
        }
        else {
            panic!("Expected \\foo");
        }

        if let Token::Command(command, args) = item3.unwrap() {
            // third command
            assert_eq!(command.name, "foo".to_string());
            assert_eq!(args.len(), 1);
            if let Token::Command(inner_command, inner_args) = &args[0] {
                assert_eq!(inner_command.name, "foo".to_string());
                assert_eq!(inner_args.len(), 1);
                if let Token::ParsedText(text) = &inner_args[0] {
                    assert_eq!("a", text);
                }
                else {
                    panic!("First argument to {} was {}", command.name, args[0]);
                }            }
            else {
                panic!("First argument to {} was {}", command.name, args[0]);
            }
        }
        else {
            panic!("Expected \\foo");
        }
    }
}