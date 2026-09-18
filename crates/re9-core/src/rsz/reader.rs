// use crate::rsz::node::{Node, NodeType};

use crate::rsz::node::{Node, NodeKind, NodeType, Value};

const ROOT_SIGNATURE: [u8; 8] = [0x01, 0x00, 0x00, 0x00, 0x3c, 0x77, 0x37, 0xe1];

fn parse_nodes(node_data: &Vec<u8>, offset: &mut usize) -> Option<Node> {
    let hash = u32::from_le_bytes(node_data[0..4].try_into().unwrap());
    let node_type = i32::from_le_bytes(node_data[4..8].try_into().unwrap());
    let size = u32::from_le_bytes(node_data[8..12].try_into().unwrap());
    let node_type = NodeType::from(node_type);

    match node_type {
        NodeType::Class => {
            let class_name = u32::from_le_bytes(node_data[12..16].try_into().unwrap());
            *offset = 16;
            let mut children = Vec::new();
            for _ in 0..size {
                let node = parse_nodes(&node_data[*offset..].try_into().unwrap(), offset);
                if let Some(n) = node {
                    children.push(n);
                }
            }
            return Some(Node {
                hash,
                node_type,
                value: Value::U32(size),
                kind: Some(NodeKind::Class {
                    class_name,
                    children,
                }),
            });
        }

        NodeType::S64 => {
            let value = i64::from_le_bytes(node_data[12..20].try_into().unwrap());
            *offset += 20;
            return Some(Node {
                hash,
                node_type,
                value: Value::S64(value),
                kind: Some(NodeKind::Number { size }),
            });
        }
        NodeType::S32 => {
            let value = i32::from_le_bytes(node_data[12..16].try_into().unwrap());
            *offset += 16;
            return Some(Node {
                hash,
                node_type,
                value: Value::S32(value),
                kind: Some(NodeKind::Number { size }),
            });
        }
        NodeType::U32 => {
            let value = u32::from_le_bytes(node_data[12..16].try_into().unwrap());
            *offset += 16;
            return Some(Node {
                hash,
                node_type,
                value: Value::U32(value),
                kind: Some(NodeKind::Number { size }),
            });
        }
        NodeType::Bool => {
            let value: bool = node_data[12] != 0;
            *offset += 16;
            return Some(Node {
                hash,
                node_type,
                value: Value::Bool(value),
                kind: Some(NodeKind::Number { size }),
            });
        }
        _ => {
            // println!("Unknown node");
        }
    }

    None
}

pub fn read(decrypted_data: &Vec<u8>) -> Vec<Node> {
    let mut roots: Vec<Node> = Vec::new();
    let mut stacked_data: Vec<u8> = Vec::new();

    // considerer de 1 decouper et 2 parse node avec chaque chunk
    let mut i = 0;
    while i < decrypted_data.len() - 8 {
        let len = roots.len();
        if len > 0 {
            stacked_data.push(decrypted_data[i]);
        }
        if decrypted_data[i..i + 8] == ROOT_SIGNATURE {
            if stacked_data.len() > 0 && len > 0 {
                // Parsing previous data (if we have any)
                let mut offset: usize = 0;
                let child_node = parse_nodes(&stacked_data, &mut offset);

                // Assign it to the previous node
                if let Some(node) = child_node {
                    roots[len - 1].kind = Some(NodeKind::Root {
                        child: Box::new(node),
                    });
                }
            }

            // Creating new node
            let hash = u32::from_le_bytes(decrypted_data[i - 4..i].try_into().unwrap());
            // let field_type = i32::from_le_bytes(decrypted_data[i..i + 4].try_into().unwrap());
            let value = u32::from_le_bytes(decrypted_data[i + 4..i + 8].try_into().unwrap());
            let root_node = Node {
                hash,
                value: Value::U32(value),
                node_type: NodeType::Root,
                kind: None,
            };
            // println!("Root found - {:?}", root_node);
            roots.push(root_node);
            i = i + 8;
        } else {
            i = i + 1;
        }
    }

    for r in roots.iter() {
        // let data_str = r.data[0..10]
        //     .iter()
        //     .map(|d| format!("{:#x}", d))
        //     .collect::<Vec<String>>()
        //     .join(", ");

        println!("{}", r);
        // println!(" Root {} data: {}", r.hash_u32, r.data.len());
    }

    roots
}

// build roots
//
// parser toutes les data, quand on rencontre une root signature, on
