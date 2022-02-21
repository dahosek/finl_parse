#![feature(assert_matches)]
use std::collections::HashMap;
use std::iter::Peekable;
use std::rc::Rc;
use std::str::CharIndices;

use unicode_categories::UnicodeCategories;

use crate::commands::{Command, Environment, ParameterFormat, ParameterType};
use crate::tokens::{Token, Location, Line, FinlError, GroupType, ErrorContext};
use std::mem;

mod tokens;
mod commands;

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


struct Parser<'a> {
    commands: HashMap<String, Rc<Command>>,
    environments: HashMap<String, Rc<Environment>>,
    lines: Box<dyn Iterator<Item=&'a str> + 'a>,
    line: Line,
    char_iterator: Peekable<CharIndices<'a>>,
    output: Vec<Result<Token, FinlError>>,
    stack: Vec<GroupType>,
}
impl<'a> Default for Parser<'a> {
    fn default() -> Self {
        Parser {
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


impl<'a> Parser<'a> {
    pub fn from_string(input: &'a str) -> Parser<'a> {
        let mut context :Parser<'a> = Parser {
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

    pub fn define_command(&mut self, name: &str, args: Vec<(ParameterFormat, ParameterType)>) {
        self.commands.insert(name.to_string(), Rc::new(Command::new(name, args)));
    }

    pub fn parse(&mut self) -> Vec<Result<Token, FinlError>> {
        self.text_parse();
        mem::take(&mut self.output)
    }

    fn push_eol_text_block(&mut self, start: usize) {
        self.push_text_block(start, self.line.contents.len());
    }
    fn push_text_block(&mut self, start: usize, end: usize) {
        if start != end {
            self.output.push(Ok(Token::ParsedText(Location::from_line_and_column(&self.line, start),
                                              self.line.contents.get(start..end).unwrap().to_string())));
        }
    }
    fn push_error(&mut self, error: FinlError) {
        self.output.push(Err(error));
    }

    fn push_command(&mut self, command: Rc<Command>, args: Vec<Token>, column: usize) {
        self.push_token(Token::Command(Location::from_line_and_column(&self.line, column),command, args))
    }

    fn push_token(&mut self, token: Token) {
        self.output.push(Ok(token))
    }

    fn skip_whitespace(&mut self) -> SkipWhiteSpaceOutcome {
        let mut new_line_count = 0usize;
        loop {
            match self.char_iterator.peek().cloned() {
                None => {
                    if self.next_line() {
                        new_line_count += 1;
                    }
                    else {
                        return SkipWhiteSpaceOutcome::EndOfFile;
                    }
                }
                Some((_, ch)) => {
                    if ch.is_whitespace() {
                        self.char_iterator.next();
                    }
                    else {
                        break;
                    }
                }
            }
        }

        if new_line_count > 1 {
            SkipWhiteSpaceOutcome::FoundBlankLine
        }
        else {
            SkipWhiteSpaceOutcome::Skipped
        }
    }

    // Get the next line. Return false if EOF on input
    fn next_line(&mut self) -> bool {
        match self.lines.next() {
            None => {
                self.line = Line::default();
                self.char_iterator = "".char_indices().peekable();
                false
            }
            Some(line) => {
                self.line.line_number += 1;
                self.line.contents = String::from(line);
                self.char_iterator = line.char_indices().peekable();
                true
            }
        }
    }

    fn undefined_command(&self, command_name: String, column: usize) -> FinlError {
        FinlError::UndefinedCommand(ErrorContext::from_line_and_column(&self.line, column),
                                    command_name)
    }

    fn unimplemented(&self, column: usize) -> FinlError {
        FinlError::Unimplemented(ErrorContext::from_line_and_column(&self.line, column))
    }

    // no column passed because it's always 0
    fn blank_line_while_parsing_command_arguments(&self, command_name: String, arg_number: usize) -> FinlError {
        FinlError::BlankLineWhileParsingCommandArguments(ErrorContext::from_line_and_column(&self.line, 0),
                                    command_name,
                                    arg_number)
    }

    // no column passed because it's always 0
    fn unexpected_eof_while_parsing_command_arguments(&self, command_name: String, arg_number: usize) -> FinlError {
        FinlError::UnexpectedEOFWhileParsingCommandArguments(ErrorContext::from_line_and_column(&self.line, 0),
                                                             command_name,
                                                             arg_number)
    }

    fn unexpected_close_brace(&self, group_type: Option<GroupType>, column: usize) -> FinlError {
        FinlError::UnexpectedCloseBrace(ErrorContext::from_line_and_column(&self.line, column), group_type)
    }


    fn text_parse(&mut self) {
        if let Some((0, _)) = self.char_iterator.peek().clone() {
            // Skip leading whitespace at beginnings of lines
            //context.char_iterator.skip_while(|(_, ch)| ch.is_whitespace());
            self.skip_whitespace();
        }
        let mut start;
        match self.char_iterator.peek() {
            None => {
                return;
            }
            Some((column, _)) => {
                start = *column;
            }
        }
        while let Some((column, ch)) = self.char_iterator.peek().cloned() {
            match ch {
                '\\' => {
                    self.push_text_block(start, column);
                    self.command_parse(CommandContext::Text);
                    start = self.get_column();
                }
                // If we have a `%`, we dump whatever's left and finish the line.
                '%' => {
                    self.push_text_block(start, column);
                    self.next_line();
                    return ;
                }
                '{' => {
                    self.push_text_block(start, column);
                    self.push_token(Token::Bgroup(Location::from_line_and_column(&self.line, column)));
                    self.char_iterator.next();
                    self.stack.push(GroupType::Brace);
                    start = self.get_column();
                }
                '}' => {
                    self.push_text_block(start, column);
                    let top_of_stack = self.stack.last().cloned();
                    if let Some(GroupType::Brace) = self.stack.pop() {
                        self.push_token(Token::Egroup(Location::from_line_and_column(&self.line, column)));
                    }
                    else if top_of_stack == Some(GroupType::RequiredArgument) {
                        return
                    }
                    else {
                        self.push_error(self.unexpected_close_brace(top_of_stack.clone(), column));
                        if let Some(group_type) = top_of_stack {
                            self.stack.push(group_type);
                        }
                    }
                    self.char_iterator.next();
                    start = self.get_column();
                }
                _ => {
                    self.char_iterator.next();
                }
            }
        }
        self.push_eol_text_block(start);
    }

    fn get_column(&mut self) -> usize {
        match self.char_iterator.peek() {
            None => {
                self.next_line();
                0
            }
            Some((column, _)) => {
                *column
            }
        }
    }

    fn command_parse(&mut self, command_context: CommandContext)  {
        let (command_start, _) = self.char_iterator.next().expect("This should not happen"); // get column of backslash
        let command_name = self.get_command_name(&command_context);
        match self.commands.get(&command_name).cloned() {
            None => {
                self.push_error(self.undefined_command(command_name, command_start));
            }
            Some(command) => {
                let mut args = Vec::with_capacity(command.parameters.len());
                let mut parameter_number = 0;
                for (format, ptype) in &command.parameters {
                    parameter_number += 1;
                    let possible_arg = match format {
                        ParameterFormat::Star => Err(self.unimplemented(command_start)),
                        ParameterFormat::Required =>
                            self.parse_required_argument(&command.name, parameter_number, &command_context, *ptype),

                        ParameterFormat::RequiredWithBraces => Err(self.unimplemented(command_start)),
                        ParameterFormat::Optional => Err(self.unimplemented(command_start)),
                        ParameterFormat::ArbitraryDelimiters => Err(self.unimplemented(command_start))
                    };
                    match possible_arg {
                        Ok(token) => {
                            args.push(token);
                        }
                        Err(err) => {
                            self.push_error(err);
                            return;
                        }
                    }
                }
                self.push_command(command.clone(), args, command_start);
            }
        }
    }

    fn get_command_name(&mut self, command_context: &CommandContext) -> String {
        let name_start = self.char_iterator.peek();
        match name_start {
            None => " ".to_string(), // backslash at end of line ≡ \␣
            Some((_, ch)) => {
                if letter_test(*ch) {
                    let mut command_name = String::new();
                    // consume letters:
                    loop {
                        if let Some((_, ch)) = self.char_iterator.peek() {
                            if letter_test(*ch) {
                                command_name.push(*ch);
                                self.char_iterator.next();
                            }
                            else {
                                // consume white space immediately after the command name
                                while let Some((_, ch)) = self.char_iterator.peek() {
                                    if ch.is_whitespace() {
                                        self.char_iterator.next();
                                    }
                                    else {
                                        break;
                                    }
                                }
                                break;
                            }
                        }
                        else {
                            // We reached the end of the line
                            break;
                        }
                    }

                    // Skip any trailing whitespace:
                    while let Some((_, ch)) = self.char_iterator.peek() {
                        if ch.is_whitespace() {
                            self.char_iterator.next();
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

    fn parse_required_argument(&mut self, command: &String, parameter_number: usize, command_context: &CommandContext, ptype: ParameterType) -> Result<Token,FinlError> {
        // white space before a required argument is ignored.
        match self.skip_whitespace() {
            SkipWhiteSpaceOutcome::Skipped => {}
            SkipWhiteSpaceOutcome::FoundBlankLine => {
                return Err(self.blank_line_while_parsing_command_arguments(command.clone(), parameter_number));
            }
            SkipWhiteSpaceOutcome::EndOfFile => {
                return Err(self.unexpected_eof_while_parsing_command_arguments(command.clone(), parameter_number));
            }
        }
        // Check next character. We know there is one from skipping whitespace.
        let (loc, ch) = self.char_iterator.peek().cloned().unwrap();
        match ch {
            '{' => {
                self.stack.push(GroupType::RequiredArgument);
                self.char_iterator.next();
                match command_context {
                    CommandContext::Text => {
                        let tokens_end = self.output.len();
                        self.text_parse();
                        let arg  = self.output.split_off(tokens_end);
                        return Err(self.unimplemented(loc));
                        // return arg.iter()
                        //     .map(|v| Token::Tokens(Location::from_line_and_column(&self.line, *loc), v))
                        // ;
                    }
                    CommandContext::UserCommandDefinition => {
                        return Err(self.unimplemented(loc));
                    }
                    CommandContext::Math => {
                        return Err(self.unimplemented(loc));
                    }
                }

                // parse to close brace
            },
            '}' => {
                // Oops this brace does not belong
            },
            '\\' => {
                // We have a command
            }
            _ => {}
        }
        // If open brace parse to close brace
        // Otherwise grab next token.

        return Err(self.unimplemented(loc))
    }





}

fn letter_test(ch: char) -> bool {
    ch.is_letter() || ch.is_mark_nonspacing() || ch.is_mark_spacing_combining()
}


#[cfg(test)]
mod test {

    use std::assert_matches::assert_matches;

    use super::*;

    // macro_rules! match_error {
    //     ($e:expr => UndefinedCommand) => {
    //         #[allow(unused_variables)]
    //         if let FinlError::UndefinedCommand(context, string) = $e {
    //             //TODO Allow a block here
    //             println!("ok!");
    //         }
    //         else {
    //             panic!("Expected UndefinedCommand for {} but found {}", stringify!($e), $e);
    //         }
    //     }
    // }

    #[test]
    fn undefined_commands_are_flagged_correctly() {
        let mut parser = Parser::from_string("\\undefined");
        let mut output = parser.parse();
        assert_eq!(output.len(), 1);
        let first_item = output.remove(0);
        let error = first_item.expect_err("First item should be an error");

        assert_matches!(error, FinlError::UndefinedCommand(_, cmd_name) if cmd_name == "undefined".to_string());

    }


    #[test]
    fn can_parse_a_command_with_no_arguments() {
        let mut parser = Parser::from_string("\\foo a");
        parser.define_command("foo", Vec::default());
        let mut output = parser.parse();
        assert_eq!(output.len(), 2);
        let item1 = output.remove(0);
        let item2 = output.remove(0);
        let command = item1.expect("First token should not be an error");
        assert_matches!(command,
            Token::Command(_, command, args)
                if command.name == "foo".to_string() && args.len() == 0
        );
        let text = item2.expect("Second token should not be an error");
        assert_matches!(text, Token::ParsedText(_, text) if text == "a".to_string());

    }

    #[test]
    fn braces_must_match() {
        let mut parser = Parser::from_string("{}");
        let mut output = parser.parse();
        assert_eq!(output.len(), 2);
        let open = output.remove(0);
        assert_matches!(open.unwrap(), Token::Bgroup(_));
        let close = output.remove(0);
        assert_matches!(close.unwrap(), Token::Egroup(_));

        let mut parser = Parser::from_string("{n}");
        let mut output = parser.parse();
        assert_eq!(output.len(), 3);
        let open = output.remove(0);
        assert_matches!(open.unwrap(), Token::Bgroup(_));
        let contents = output.remove(0);
        assert_matches!(contents.unwrap(), Token::ParsedText(_, "n".to_string()));
        let close = output.remove(0);
        assert_matches!(close.unwrap(), Token::Egroup(_));

        let mut parser = Parser::from_string("{\n}"); // Really? I want to ignore a blank line after an opening brace?
        let mut output = parser.parse();
        assert_eq!(output.len(), 2);
        let open = output.remove(0);
        assert_matches!(open.unwrap(), Token::Bgroup(_));
        let close = output.remove(0);
        assert_matches!(close.unwrap(), Token::Egroup(_));

        let mut parser = Parser::from_string("{}}");
        let mut output = parser.parse();
        assert_eq!(output.len(), 3);
        let open = output.remove(0);
        assert_matches!(open.unwrap(), Token::Bgroup(_));
        let close = output.remove(0);
        assert_matches!(close.unwrap(), Token::Egroup(_));
        let err = output.remove(0);
        assert_matches!(err.unwrap_err(), FinlError::UnexpectedCloseBrace(_, group_type) if group_type == None);
    }

    /*

#[test]
fn braces_tokenize_correctly_with_text() {
    let mut context = Parser::from_string("{abc}");
    let mut output = context.parse();
    println!("{:?}", output);
    assert_eq!(output.len(), 3);
    let (item, _loc) = output.remove(0);
    assert_eq!(Ok(Token::Bgroup), item);
    let (item, _loc) = output.remove(0);
    assert_eq!(Ok(Token::ParsedText("abc".to_string())), item);
    let (item, _loc) = output.remove(0);
    assert_eq!(Ok(Token::Egroup), item);

}

#[test]
fn skip_white_space_finds_blank_line() {
    let mut context = Parser::from_string("\n\na");
    assert_eq!(context.skip_whitespace(), SkipWhiteSpaceOutcome::FoundBlankLine);
}

#[test]
fn blank_line_before_argument_is_error() {
    let mut parser = Parser::from_string("\\foo\n\n{a}");
    parser.define_command("foo", vec![(ParameterFormat::Required, ParameterType::ParsedTokens)]);
    let mut output = parser.parse();
    println!("{:?}", output);
    assert_eq!(output.len(), 4);
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

 */
}