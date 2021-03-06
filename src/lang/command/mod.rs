mod closure;

use crate::lang::argument::ArgumentDefinition;
use crate::lang::errors::{error, CrushResult};
use crate::lang::execution_context::{CompileContext, CommandContext};
use crate::lang::help::Help;
use crate::lang::job::Job;
use crate::lang::scope::Scope;
use crate::lang::serialization::model;
use crate::lang::serialization::model::{element, Element, Strings};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::value::{Value, ValueDefinition, ValueType};
use closure::Closure;
use ordered_map::OrderedMap;
use std::fmt::{Formatter, Display};

pub type Command = Box<dyn CrushCommand + Send + Sync>;

#[derive(Clone, Debug)]
pub enum OutputType {
    Unknown,
    Known(ValueType),
    Passthrough,
}

impl OutputType {
    fn calculate<'a>(&'a self, input: &'a OutputType) -> Option<&'a ValueType> {
        match self {
            OutputType::Unknown => None,
            OutputType::Known(t) => Some(t),
            OutputType::Passthrough => input.calculate(&OutputType::Unknown),
        }
    }

    fn format(&self) -> Option<String> {
        match self {
            OutputType::Unknown => None,
            OutputType::Known(t) => Some(format!("    Output: {}", t)),
            OutputType::Passthrough => {
                Some("    Output: A stream with the same columns as the input".to_string())
            }
        }
    }
}

pub trait CrushCommand: Help {
    fn invoke(&self, context: CommandContext) -> CrushResult<()>;
    fn can_block(&self, arguments: &[ArgumentDefinition], context: &mut CompileContext) -> bool;
    fn name(&self) -> &str;
    fn copy(&self) -> Command;
    fn help(&self) -> &dyn Help;
    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize>;
    fn bind(&self, this: Value) -> Command;
    fn output<'a>(&'a self, input: &'a OutputType) -> Option<&'a ValueType>;
}

pub trait TypeMap {
    fn declare(
        &mut self,
        path: Vec<&str>,
        call: fn(context: CommandContext) -> CrushResult<()>,
        can_block: bool,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
        output: OutputType,
    );
}

impl TypeMap for OrderedMap<String, Command> {
    fn declare(
        &mut self,
        path: Vec<&str>,
        call: fn(CommandContext) -> CrushResult<()>,
        can_block: bool,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
        output: OutputType,
    ) {
        self.insert(
            path[path.len() - 1].to_string(),
            CrushCommand::command(
                call,
                can_block,
                path.iter().map(|e| e.to_string()).collect(),
                signature,
                short_help,
                long_help,
                output,
            ),
        );
    }
}

struct SimpleCommand {
    call: fn(context: CommandContext) -> CrushResult<()>,
    can_block: bool,
    full_name: Vec<String>,
    signature: &'static str,
    short_help: &'static str,
    long_help: Option<&'static str>,
    output: OutputType,
}

struct ConditionCommand {
    call: fn(context: CommandContext) -> CrushResult<()>,
    full_name: Vec<String>,
    signature: &'static str,
    short_help: &'static str,
    long_help: Option<&'static str>,
}

impl dyn CrushCommand {
    pub fn closure(
        name: Option<String>,
        signature: Option<Vec<Parameter>>,
        job_definitions: Vec<Job>,
        env: &Scope,
    ) -> Command {
        Box::from(Closure::new(name, signature, job_definitions, env.clone()))
    }

    pub fn command(
        call: fn(context: CommandContext) -> CrushResult<()>,
        can_block: bool,
        full_name: Vec<String>,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
        output: OutputType,
    ) -> Command {
        Box::from(SimpleCommand {
            call,
            can_block,
            full_name,
            signature,
            short_help,
            long_help,
            output,
        })
    }

    pub fn condition(
        call: fn(context: CommandContext) -> CrushResult<()>,
        full_name: Vec<String>,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
    ) -> Command {
        Box::from(ConditionCommand {
            call,
            full_name,
            signature,
            short_help,
            long_help,
        })
    }

    pub fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<Command> {
        match elements[id].element.as_ref().unwrap() {
            element::Element::Command(strings) => {
                let val = state
                    .env
                    .global_value(strings.elements.iter().map(|e| e.clone()).collect())?;
                match val {
                    Value::Command(c) => Ok(c),
                    _ => error("Expected a command"),
                }
            }
            element::Element::BoundCommand(bound_command) => {
                let this = Value::deserialize(bound_command.this as usize, elements, state)?;
                let command =
                    CrushCommand::deserialize(bound_command.command as usize, elements, state)?;
                Ok(command.bind(this))
            }
            element::Element::Closure(_) => Closure::deserialize(id, elements, state),
            _ => error("Expected a command"),
        }
    }
}

impl CrushCommand for SimpleCommand {
    fn invoke(&self, context: CommandContext) -> CrushResult<()> {
        let c = self.call;
        c(context)
    }

    fn name(&self) -> &str {
        "command"
    }

    fn can_block(&self, _arg: &[ArgumentDefinition], _context: &mut CompileContext) -> bool {
        self.can_block
    }

    fn copy(&self) -> Command {
        Box::from(SimpleCommand {
            call: self.call,
            can_block: self.can_block,
            full_name: self.full_name.clone(),
            signature: self.signature,
            short_help: self.short_help,
            long_help: self.long_help,
            output: self.output.clone(),
        })
    }

    fn help(&self) -> &dyn Help {
        self
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        _state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let idx = elements.len();
        elements.push(Element {
            element: Some(element::Element::Command(Strings {
                elements: self.full_name.iter().map(|e| e.to_string()).collect(),
            })),
        });
        Ok(idx)
    }

    fn bind(&self, this: Value) -> Command {
        Box::from(BoundCommand {
            command: self.copy(),
            this,
        })
    }

    fn output<'a>(&'a self, input: &'a OutputType) -> Option<&'a ValueType> {
        self.output.calculate(input)
    }
}

impl Help for SimpleCommand {
    fn signature(&self) -> String {
        self.signature.to_string()
    }

    fn short_help(&self) -> String {
        self.short_help.to_string()
    }

    fn long_help(&self) -> Option<String> {
        let output = self.output.format();
        let long_cat = self.long_help.map(|s| s.to_string());
        match (output, long_cat) {
            (Some(o), Some(l)) => Some(format!("{}\n\n{}", o, l)),
            (Some(o), None) => Some(o),
            (None, Some(o)) => Some(o),
            (None, None) => None,
        }
    }
}

impl std::cmp::PartialEq for SimpleCommand {
    fn eq(&self, _other: &SimpleCommand) -> bool {
        false
    }
}

impl std::cmp::Eq for SimpleCommand {}

impl std::fmt::Debug for SimpleCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Command")
    }
}

impl CrushCommand for ConditionCommand {
    fn invoke(&self, context: CommandContext) -> CrushResult<()> {
        let c = self.call;
        c(context)
    }

    fn name(&self) -> &str {
        "conditional command"
    }

    fn can_block(&self, arguments: &[ArgumentDefinition], context: &mut CompileContext) -> bool {
        arguments
            .iter()
            .any(|arg| arg.value.can_block(arguments, context))
    }

    fn copy(&self) -> Command {
        Box::from(ConditionCommand {
            call: self.call,
            full_name: self.full_name.clone(),
            signature: self.signature,
            short_help: self.short_help,
            long_help: self.long_help,
        })
    }

    fn help(&self) -> &dyn Help {
        self
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        _state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let idx = elements.len();
        elements.push(Element {
            element: Some(element::Element::Command(Strings {
                elements: self.full_name.iter().map(|e| e.to_string()).collect(),
            })),
        });
        Ok(idx)
    }

    fn bind(&self, this: Value) -> Command {
        Box::from(BoundCommand {
            command: self.copy(),
            this,
        })
    }

    fn output(&self, _input: &OutputType) -> Option<&ValueType> {
        None
    }
}

impl Help for ConditionCommand {
    fn signature(&self) -> String {
        self.signature.to_string()
    }

    fn short_help(&self) -> String {
        self.short_help.to_string()
    }

    fn long_help(&self) -> Option<String> {
        self.long_help.map(|s| s.to_string())
    }
}

impl std::cmp::PartialEq for ConditionCommand {
    fn eq(&self, _other: &ConditionCommand) -> bool {
        false
    }
}

impl std::cmp::Eq for ConditionCommand {}

#[derive(Clone)]
pub enum Parameter {
    Parameter(String, ValueDefinition, Option<ValueDefinition>),
    Named(String),
    Unnamed(String),
}

impl Display for Parameter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Parameter::Parameter(name, value_type, default) => {
                name.fmt(f)?;
                f.write_str(":")?;
                value_type.fmt(f)?;
                if let Some(default) = default {
                    f.write_str("=")?;
                    default.fmt(f)?;
                }
                Ok(())
            }
            Parameter::Named(n) => {
                f.write_str("@@")?;
                n.fmt(f)
            }
            Parameter::Unnamed(n) => {
                f.write_str("@")?;
                n.fmt(f)
            }
        }
    }
}

pub struct BoundCommand {
    command: Command,
    this: Value,
}

impl CrushCommand for BoundCommand {
    fn invoke(&self, mut context: CommandContext) -> CrushResult<()> {
        context.this = Some(self.this.clone());
        self.command.invoke(context)
    }

    fn can_block(&self, arguments: &[ArgumentDefinition], context: &mut CompileContext) -> bool {
        self.command.can_block(arguments, context)
    }

    fn name(&self) -> &str {
        self.command.name()
    }

    fn copy(&self) -> Command {
        Box::from(BoundCommand {
            command: self.command.copy(),
            this: self.this.clone(),
        })
    }

    fn help(&self) -> &dyn Help {
        self.command.help()
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let this = self.this.serialize(elements, state)? as u64;
        let command = self.command.serialize(elements, state)? as u64;
        let idx = elements.len();
        elements.push(Element {
            element: Some(element::Element::BoundCommand(model::BoundCommand {
                this,
                command,
            })),
        });
        Ok(idx)
    }

    fn bind(&self, this: Value) -> Command {
        Box::from(BoundCommand {
            command: self.command.copy(),
            this: this.clone(),
        })
    }

    fn output<'a>(&'a self, input: &'a OutputType) -> Option<&'a ValueType> {
        self.command.output(input)
    }
}

impl Help for BoundCommand {
    fn signature(&self) -> String {
        self.command.signature()
    }

    fn short_help(&self) -> String {
        self.command.short_help()
    }

    fn long_help(&self) -> Option<String> {
        self.command.long_help()
    }
}
