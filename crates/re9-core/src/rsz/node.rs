// [hash u32][field_type i32][value]
// <native_hash> 01000000 3c7737e1

use std::fmt;

#[derive(Debug)]
pub enum Value {
    U32(u32),
    S32(i32),
    S64(i64),
    Bool(bool),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            Value::U32(x) => write!(f, "{:#x} ({})", x, x),
            Value::S32(x) => write!(f, "{:#x} ({})", x, x),
            Value::S64(x) => write!(f, "{:#x} ({})", x, x),
            Value::Bool(x) => write!(f, "{}", x),

            _ => write!(f, "- Not implemented yet"),
        }
    }
}

pub struct Node {
    pub hash: u32,
    pub value: Value,
    pub node_type: NodeType,
    pub kind: Option<NodeKind>,
}

pub enum NodeKind {
    Class {
        class_name: u32,
        children: Vec<Node>,
    },
    Number {
        size: u32,
    },
    Array {
        elements: Vec<Node>,
    },
    Root {
        child: Box<Node>,
    },
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            Some(NodeKind::Root { child }) => {
                write!(
                    f,
                    "{:?}[{:#x}] - value {}\n",
                    self.node_type, self.hash, self.value
                )?;
                write!(f, "- {}", child)
            }
            Some(NodeKind::Class {
                class_name,
                children,
            }) => {
                write!(
                    f,
                    "Class[{:#x}] Name: {:#x} - {} fields\n",
                    self.hash,
                    class_name,
                    children.len()
                )?;
                children.iter().try_for_each(|c| write!(f, " |- {}", c))
            }
            Some(NodeKind::Number { size }) => {
                write!(
                    f,
                    "{:?}[{:#x}] value: {} Size: {}\n",
                    self.node_type, self.hash, self.value, size
                )
            }

            _ => write!(f, "- Not implemented yet"),
        }
    }
}

// #[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Root = -2,
    Array = -1,
    Unknown = 0,
    Enum = 1,
    Bool = 2,
    S8 = 3,
    U8 = 4,
    S16 = 5,
    U16 = 6,
    S32 = 7,
    U32 = 8,
    S64 = 9,
    U64 = 0xa,
    F32 = 0xb,
    F64 = 0xc,
    C8 = 0xd,
    C16 = 0xe,
    String = 0xf,
    Struct = 0x10,
    Class = 0x11,
}

impl From<i32> for NodeType {
    fn from(t: i32) -> Self {
        match t {
            -1 => NodeType::Array,
            0 => NodeType::Unknown,
            1 => NodeType::Enum,
            2 => NodeType::Bool,
            3 => NodeType::S8,
            4 => NodeType::U8,
            5 => NodeType::S16,
            6 => NodeType::U16,
            7 => NodeType::S32,
            8 => NodeType::U32,
            9 => NodeType::S64,
            0xa => NodeType::U64,
            0xb => NodeType::F32,
            0xc => NodeType::F64,
            0xd => NodeType::C8,
            0xe => NodeType::C16,
            0xf => NodeType::String,
            0x10 => NodeType::Struct,
            0x11 => NodeType::Class,
            _ => NodeType::Unknown, // fallback for unrecognized values
        }
    }
}
