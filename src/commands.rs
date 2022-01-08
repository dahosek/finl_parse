
#[derive(Debug, PartialEq)]
pub struct Command {
    pub name: String,
    pub parameters: Vec<(ParameterFormat, ParameterType)>,
    //TODO: Function pointer for execution
}

impl Command {
    pub fn new(name: &str, args: Vec<(ParameterFormat, ParameterType)>) -> Command {
        Command {
            name: name.to_string(),
            parameters: args
        }
    }
    
    pub fn no_arg_command(name: String) -> Command {
        Command {
            name,
            parameters: Vec::default()
        }
    }
    
    
}

#[derive(Debug, PartialEq)]
pub struct Environment {
    pub name: String,
    pub args: Vec<(ParameterFormat, ParameterType)>,
    pub body_type: ParameterType
    // TODO: Function pointer for execution
}

#[derive(Debug, PartialEq)]
pub enum ParameterFormat {
    Star,
    Required,
    RequiredWithBraces,
    Optional,
    ArbitraryDelimiters,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ParameterType {
    ParsedTokens,
    VerbatimText,
    Boolean,
    KeyValueList,
    MacroDefinition,
    Math,
    YAML,
}