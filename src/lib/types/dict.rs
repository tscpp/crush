use crate::lang::command::Command;
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error, CrushResult};
use crate::lang::execution_context::{ArgumentVector, CommandContext, This};
use crate::lang::value::Value;
use crate::lang::{dict::Dict, value::ValueType};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "dict", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        res.declare(
            full("new"),
            new,
            false,
            "dict:new",
            "Construct a new dict",
            Some(
                r#"    Examples:
    my_dict := (dict string integer):new"#,
            ),
            Unknown,
        );
        res.declare(
            full("len"),
            len,
            false,
            "dict:len",
            "The number of mappings in the dict",
            None,
            Known(ValueType::Integer),
        );
        res.declare(
            full("empty"),
            empty,
            false,
            "dict:empty",
            "True if there are no mappings in the dict",
            None,
            Known(ValueType::Bool),
        );
        res.declare(
            full("clear"),
            clear,
            false,
            "dict:clear",
            "Remove all mappings from this dict",
            None,
            Unknown,
        );
        res.declare(
            full("__setitem__"),
            setitem,
            false,
            "dict[key] = value",
            "Create a new mapping or replace an existing one",
            None,
            Unknown,
        );
        res.declare(
            full("__getitem__"),
            getitem,
            false,
            "dict[key]",
            "Return the value the specified key is mapped to",
            None,
            Unknown,
        );
        res.declare(
            full("remove"),
            remove,
            false,
            "dict:remove key",
            "Remove a mapping from the dict",
            None,
            Unknown,
        );
        res.declare(
            full("clone"),
            clone,
            false,
            "dict:clone",
            "Create a new dict with the same st of mappings as this one",
            None,
            Unknown,
        );
        res.declare(
            full("__call__"),
            call_type,
            false,
            "dict key_type:type value_type:type",
            "Returns a dict type with the specifiec key and value types",
            None,
            Known(ValueType::Type),
        );
        res.declare(
            full("key_type"),
            key_type,
            false,
            "dict:key_type",
            "Return the type of the keys in this dict",
            None,
            Known(ValueType::Type),
        );
        res.declare(
            full("value_type"),
            value_type,
            false,
            "dict:value_type",
            "Return the type of the values in this dict",
            None,
            Known(ValueType::Type),
        );
        res
    };
}

fn call_type(mut context: CommandContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::Dict(t1, t2) => match (*t1, *t2) {
            (ValueType::Empty, ValueType::Empty) => {
                context.arguments.check_len(2)?;
                let key_type = context.arguments.r#type(0)?;
                let value_type = context.arguments.r#type(1)?;
                context.output.send(Value::Type(ValueType::Dict(
                    Box::new(key_type),
                    Box::new(value_type),
                )))
            }
            (t1, t2) => {
                if context.arguments.is_empty() {
                    context
                        .output
                        .send(Value::Type(ValueType::Dict(Box::from(t1), Box::from(t2))))
                } else {
                    argument_error("Tried to set subtype on a dict that already has a subtype")
                }
            }
        },
        _ => argument_error("Invalid this, expected type dict"),
    }
}

fn new(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let t = context.this.r#type()?;
    if let ValueType::Dict(key_type, value_type) = t {
        if !key_type.is_hashable() {
            argument_error("Key type is not hashable")
        } else {
            context
                .output
                .send(Value::Dict(Dict::new(*key_type, *value_type)))
        }
    } else {
        argument_error("Expected a dict type as this value")
    }
}

fn setitem(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let dict = context.this.dict()?;
    let value = context.arguments.value(1)?;
    let key = context.arguments.value(0)?;
    dict.insert(key, value)
}

fn getitem(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let dict = context.this.dict()?;
    let key = context.arguments.value(0)?;
    let o = context.output;
    dict.get(&key).map(|c| o.send(c));
    Ok(())
}

fn remove(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let dict = context.this.dict()?;
    let key = context.arguments.value(0)?;
    let o = context.output;
    dict.remove(&key).map(|c| o.send(c));
    Ok(())
}

fn len(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(context.this.dict()?.len() as i128))
}

fn clear(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let d = context.this.dict()?;
    d.clear();
    context.output.send(Value::Dict(d))
}

fn clone(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let d = context.this.dict()?;
    context.output.send(Value::Dict(d.copy()))
}

fn empty(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Bool(context.this.dict()?.len() == 0))
}

fn key_type(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Type(context.this.dict()?.key_type()))
}

fn value_type(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Type(context.this.dict()?.value_type()))
}
