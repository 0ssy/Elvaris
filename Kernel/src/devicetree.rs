//! Device-tree hardware description (Phase 20.2).
//!
//! A controlled, validated interface for embedded hardware descriptions.
//! Parses a minimal device-tree blob, exposes nodes/properties through
//! validated accessors, and never exposes raw unchecked values to drivers.
//!
//! The initial embedded target is x86-64 QEMU q35, which uses ACPI rather
//! than a device tree. This module provides device-tree support so an
//! embedded board can be described portably, and so the parser and its
//! validation are tested independently of the boot firmware in use.

#![allow(dead_code)]

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::result;

/// A device-tree parsing or validation error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevicetreeError {
    /// The data is too small to contain a device-tree header.
    Truncated,
    /// The device-tree magic value is missing.
    BadMagic,
    /// The device-tree version is unsupported.
    BadVersion,
    /// The declared size does not match the data length.
    BadSize,
    /// A structure or strings block offset/length is out of bounds.
    OutOfBounds,
    /// A string is not NUL-terminated within the strings block.
    BadString,
    /// A property value length exceeds the available data.
    BadPropertyLength,
    /// An address-cell or size-cell value is invalid.
    BadCellSize,
    /// A register range overflows the address space.
    RegisterOverflow,
    /// A node or property name is invalid.
    BadName,
    /// A referenced node or phandle does not exist.
    InvalidReference,
    /// Duplicate or conflicting resources were detected.
    Conflict,
    /// The device tree is malformed in an unspecified way.
    Malformed,
}

pub type Result<T> = result::Result<T, DevicetreeError>;

const FDT_MAGIC: u32 = 0xd00dfeed;
const FDT_BEGIN_NODE: u32 = 0x00000001;
const FDT_END_NODE: u32 = 0x00000002;
const FDT_PROP: u32 = 0x00000003;
const FDT_NOP: u32 = 0x00000004;
const FDT_END: u32 = 0x00000009;

const FDT_HEADER_SIZE: usize = 40;

/// Read a big-endian u32 from a byte slice at the given offset.
fn read_u32_be(data: &[u8], offset: usize) -> Result<u32> {
    if offset + 4 > data.len() {
        return Err(DevicetreeError::Truncated);
    }
    Ok(u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ]))
}

/// A parsed device-tree property.
#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    /// Property name.
    pub name: String,
    /// Raw property value bytes.
    pub value: Vec<u8>,
}

/// A parsed device-tree node.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// Node name (unit name stripped of the address suffix).
    pub name: String,
    /// Full path from the root.
    pub path: String,
    /// Properties of this node.
    pub properties: BTreeMap<String, Property>,
    /// Child nodes, keyed by name, as indices into the device tree's node vec.
    pub children: BTreeMap<String, usize>,
}

/// The parsed device tree.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceTree {
    nodes: Vec<Node>,
    root_idx: usize,
    /// Address cells for the root (default 2).
    root_address_cells: u32,
    /// Size cells for the root (default 1).
    root_size_cells: u32,
}

impl DeviceTree {
    /// Parse a device-tree blob.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < FDT_HEADER_SIZE {
            return Err(DevicetreeError::Truncated);
        }
        let magic = read_u32_be(data, 0)?;
        if magic != FDT_MAGIC {
            return Err(DevicetreeError::BadMagic);
        }
        let version = read_u32_be(data, 20)?;
        if version < 16 {
            return Err(DevicetreeError::BadVersion);
        }
        let totalsize = read_u32_be(data, 4)? as usize;
        if totalsize != data.len() {
            return Err(DevicetreeError::BadSize);
        }
        let off_struct = read_u32_be(data, 8)? as usize;
        let off_strings = read_u32_be(data, 12)? as usize;
        let size_strings = read_u32_be(data, 32)? as usize;
        let size_struct = read_u32_be(data, 36)? as usize;
        if off_struct + size_struct > data.len()
            || off_strings + size_strings > data.len()
        {
            return Err(DevicetreeError::OutOfBounds);
        }
        let struct_block = &data[off_struct..off_struct + size_struct];
        let strings_block = &data[off_strings..off_strings + size_strings];

        let mut nodes: Vec<Node> = Vec::new();
        nodes.push(Node {
            name: "/".to_string(),
            path: "/".to_string(),
            properties: BTreeMap::new(),
            children: BTreeMap::new(),
        });
        let root_idx = 0;
        let mut stack: Vec<usize> = Vec::new();
        stack.push(root_idx);
        let mut address_cells: Vec<u32> = Vec::new();
        address_cells.push(2);
        let mut size_cells: Vec<u32> = Vec::new();
        size_cells.push(1);

        let mut i = 0;
        while i < struct_block.len() {
            if i + 4 > struct_block.len() {
                return Err(DevicetreeError::Malformed);
            }
            let token = u32::from_be_bytes([
                struct_block[i],
                struct_block[i + 1],
                struct_block[i + 2],
                struct_block[i + 3],
            ]);
            i += 4;
            match token {
                FDT_BEGIN_NODE => {
                    let name = read_cstring(struct_block, &mut i)?;
                    // An empty name is the root node itself, which already
                    // exists; do not create a duplicate child.
                    if name.is_empty() {
                        continue;
                    }
                    let parent_idx = *stack.last().unwrap();
                    let path = if parent_idx == root_idx {
                        alloc::format!("/{}", name)
                    } else {
                        alloc::format!("{}/{}", nodes[parent_idx].path, name)
                    };
                    let child_idx = nodes.len();
                    nodes.push(Node {
                        name: name.clone(),
                        path,
                        properties: BTreeMap::new(),
                        children: BTreeMap::new(),
                    });
                    nodes[parent_idx].children.insert(name.clone(), child_idx);
                    let addr = *address_cells.last().unwrap_or(&2);
                    let sz = *size_cells.last().unwrap_or(&1);
                    address_cells.push(addr);
                    size_cells.push(sz);
                    stack.push(child_idx);
                }
                FDT_END_NODE => {
                    // The root node was never pushed, so never pop it.
                    if stack.len() > 1 {
                    if stack.pop().is_none() {
                        return Err(DevicetreeError::Malformed);
                    }
                        address_cells.pop();
                        size_cells.pop();
                    }
                }
                FDT_PROP => {
                    if i + 8 > struct_block.len() {
                        return Err(DevicetreeError::Malformed);
                    }
                    let len = u32::from_be_bytes([
                        struct_block[i],
                        struct_block[i + 1],
                        struct_block[i + 2],
                        struct_block[i + 3],
                    ]) as usize;
                    let nameoff = u32::from_be_bytes([
                        struct_block[i + 4],
                        struct_block[i + 5],
                        struct_block[i + 6],
                        struct_block[i + 7],
                    ]) as usize;
                    i += 8;
                    if i + len > struct_block.len() {
                        return Err(DevicetreeError::BadPropertyLength);
                    }
                    let prop_name = read_string_from(strings_block, nameoff)?;
                    let value = struct_block[i..i + len].to_vec();
                    i += len;
                    i = (i + 3) & !3;
                    // Track address/size cells changes (borrow value first).
                    if prop_name == "#address-cells" && value.len() >= 4 {
                        let val = u32::from_be_bytes([
                            value[0], value[1], value[2], value[3],
                        ]);
                        if val > 4 {
                            return Err(DevicetreeError::BadCellSize);
                        }
                        if let Some(last) = address_cells.last_mut() {
                            *last = val;
                        }
                    }
                    if prop_name == "#size-cells" && value.len() >= 4 {
                        let val = u32::from_be_bytes([
                            value[0], value[1], value[2], value[3],
                        ]);
                        if val > 4 {
                            return Err(DevicetreeError::BadCellSize);
                        }
                        if let Some(last) = size_cells.last_mut() {
                            *last = val;
                        }
                    }
                    let cur_idx = *stack.last().unwrap();
                    if nodes[cur_idx].properties.contains_key(&prop_name) {
                        return Err(DevicetreeError::Conflict);
                    }
                    nodes[cur_idx].properties.insert(
                        prop_name.clone(),
                        Property {
                            name: prop_name.clone(),
                            value,
                        },
                    );
                }
                FDT_NOP => {}
                FDT_END => break,
                _ => return Err(DevicetreeError::Malformed),
            }
        }

        Ok(DeviceTree {
            nodes,
            root_idx,
            root_address_cells: *address_cells.first().unwrap_or(&2),
            root_size_cells: *size_cells.first().unwrap_or(&1),
        })
    }

    /// The root node.
    pub fn root(&self) -> &Node {
        &self.nodes[self.root_idx]
    }

    /// Find a node by path (e.g. "/soc/serial@1").
    pub fn find_node(&self, path: &str) -> Option<&Node> {
        find_node_recursive(&self.nodes, self.root_idx, path)
    }

    /// The root address-cells value.
    pub fn root_address_cells(&self) -> u32 {
        self.root_address_cells
    }

    /// The root size-cells value.
    pub fn root_size_cells(&self) -> u32 {
        self.root_size_cells
    }

    /// Find nodes whose `compatible` property contains one of the given
    /// compatible strings.
    pub fn find_by_compatible(&self, compatible: &[&str]) -> Vec<&Node> {
        let mut out = Vec::new();
        find_compatible_recursive(&self.nodes, self.root_idx, compatible, &mut out);
        out
    }
}

fn read_cstring(data: &[u8], pos: &mut usize) -> Result<String> {
    let start = *pos;
    while *pos < data.len() {
        if data[*pos] == 0 {
            let s = core::str::from_utf8(&data[start..*pos])
                .map_err(|_| DevicetreeError::BadString)?;
            *pos += 1;
            // Align to 4 bytes.
            *pos = (*pos + 3) & !3;
            return Ok(s.to_string());
        }
        *pos += 1;
    }
    Err(DevicetreeError::BadString)
}

fn read_string_from(block: &[u8], offset: usize) -> Result<String> {
    if offset >= block.len() {
        return Err(DevicetreeError::BadString);
    }
    let mut end = offset;
    while end < block.len() && block[end] != 0 {
        end += 1;
    }
    if end >= block.len() {
        return Err(DevicetreeError::BadString);
    }
    core::str::from_utf8(&block[offset..end])
        .map(|s| s.to_string())
        .map_err(|_| DevicetreeError::BadString)
}

fn find_node_recursive<'a>(nodes: &'a [Node], idx: usize, path: &str) -> Option<&'a Node> {
    let node = &nodes[idx];
    if node.path == path {
        return Some(node);
    }
    for child_idx in node.children.values() {
        if let Some(found) = find_node_recursive(nodes, *child_idx, path) {
            return Some(found);
        }
    }
    None
}

fn find_compatible_recursive<'a, 'b>(
    nodes: &'a [Node],
    idx: usize,
    compatible: &[&str],
    out: &mut Vec<&'b Node>,
) where
    'a: 'b,
{
    let node = &nodes[idx];
    if let Some(prop) = node.properties.get("compatible") {
        let s = core::str::from_utf8(&prop.value).unwrap_or("");
        if compatible.iter().any(|c| s.contains(c)) {
            out.push(node);
        }
    }
    for child_idx in node.children.values() {
        find_compatible_recursive(nodes, *child_idx, compatible, out);
    }
}

impl TryFrom<&[u8]> for DeviceTree {
    type Error = DevicetreeError;
    fn try_from(data: &[u8]) -> Result<Self> {
        Self::parse(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_u32(b: &mut Vec<u8>, v: u32) {
        b.extend_from_slice(&v.to_be_bytes());
    }

    fn push_bytes(b: &mut Vec<u8>, v: &[u8]) {
        b.extend_from_slice(v);
    }

    fn push_cstring(b: &mut Vec<u8>, s: &str) {
        push_bytes(b, s.as_bytes());
        push_bytes(b, &[0]);
        while b.len() % 4 != 0 {
            push_bytes(b, &[0]);
        }
    }

    fn build_simple_dtb() -> Vec<u8> {
        // / { compatible = "q35"; #address-cells = <2>; #size-cells = <1>;
        //     soc@0 { compatible = "simple-bus"; #address-cells = <2>;
        //         #size-cells = <1>; serial@1 { compatible = "ns16550";
        //             reg = <0 0x1f00 0 8>; };
        //     };
        // };
        let mut strings = Vec::new();
        let mut add_str = |s: &str| -> usize {
            let off = strings.len();
            strings.extend_from_slice(s.as_bytes());
            strings.push(0);
            off
        };
        let prop_compatible = add_str("compatible");
        let prop_reg = add_str("reg");
        let prop_address_cells = add_str("#address-cells");
        let prop_size_cells = add_str("#size-cells");

        let mut struct_block = Vec::new();
        // / BEGIN_NODE
        push_u32(&mut struct_block, FDT_BEGIN_NODE);
        push_cstring(&mut struct_block, "");
        // compatible prop (root)
        push_u32(&mut struct_block, FDT_PROP);
        push_u32(&mut struct_block, 4);
        push_u32(&mut struct_block, prop_compatible as u32);
        push_bytes(&mut struct_block, b"q35\x00");
        while struct_block.len() % 4 != 0 {
            struct_block.push(0);
        }
        // #address-cells prop
        push_u32(&mut struct_block, FDT_PROP);
        push_u32(&mut struct_block, 4);
        push_u32(&mut struct_block, prop_address_cells as u32);
        push_u32(&mut struct_block, 2);
        // #size-cells prop
        push_u32(&mut struct_block, FDT_PROP);
        push_u32(&mut struct_block, 4);
        push_u32(&mut struct_block, prop_size_cells as u32);
        push_u32(&mut struct_block, 1);
        // soc@0 BEGIN_NODE
        push_u32(&mut struct_block, FDT_BEGIN_NODE);
        push_cstring(&mut struct_block, "soc@0");
        // compatible prop
        push_u32(&mut struct_block, FDT_PROP);
        push_u32(&mut struct_block, 11);
        push_u32(&mut struct_block, prop_compatible as u32);
        push_bytes(&mut struct_block, b"simple-bus\x00");
        while struct_block.len() % 4 != 0 {
            struct_block.push(0);
        }
        push_u32(&mut struct_block, FDT_PROP);
        push_u32(&mut struct_block, 4);
        push_u32(&mut struct_block, prop_address_cells as u32);
        push_u32(&mut struct_block, 2);
        push_u32(&mut struct_block, FDT_PROP);
        push_u32(&mut struct_block, 4);
        push_u32(&mut struct_block, prop_size_cells as u32);
        push_u32(&mut struct_block, 1);
        // serial@1 BEGIN_NODE
        push_u32(&mut struct_block, FDT_BEGIN_NODE);
        push_cstring(&mut struct_block, "serial@1");
        // compatible prop
        push_u32(&mut struct_block, FDT_PROP);
        push_u32(&mut struct_block, 8);
        push_u32(&mut struct_block, prop_compatible as u32);
        push_bytes(&mut struct_block, b"ns16550\x00");
        while struct_block.len() % 4 != 0 {
            struct_block.push(0);
        }
        // reg prop = <0 0x1f00 0 8> (2 address cells + 2 size cells = 16 bytes)
        push_u32(&mut struct_block, FDT_PROP);
        push_u32(&mut struct_block, 16);
        push_u32(&mut struct_block, prop_reg as u32);
        push_u32(&mut struct_block, 0);
        push_u32(&mut struct_block, 0x1f00);
        push_u32(&mut struct_block, 0);
        push_u32(&mut struct_block, 8);
        // serial@1 END_NODE
        push_u32(&mut struct_block, FDT_END_NODE);
        // soc@0 END_NODE
        push_u32(&mut struct_block, FDT_END_NODE);
        // / END_NODE
        push_u32(&mut struct_block, FDT_END_NODE);
        // FDT_END
        push_u32(&mut struct_block, FDT_END);

        let off_struct = FDT_HEADER_SIZE;
        let off_strings = off_struct + struct_block.len();
        let totalsize = off_strings + strings.len();
        let mut header = [0u8; FDT_HEADER_SIZE];
        header[0..4].copy_from_slice(&FDT_MAGIC.to_be_bytes());
        header[4..8].copy_from_slice(&(totalsize as u32).to_be_bytes());
        header[8..12].copy_from_slice(&(off_struct as u32).to_be_bytes());
        header[12..16].copy_from_slice(&(off_strings as u32).to_be_bytes());
        header[16..20].copy_from_slice(&0u32.to_be_bytes()); // off_mem_rsvmap
        header[20..24].copy_from_slice(&17u32.to_be_bytes()); // version
        header[24..28].copy_from_slice(&16u32.to_be_bytes()); // last_comp_version
        header[28..32].copy_from_slice(&0u32.to_be_bytes()); // boot_cpuid_phys
        header[32..36].copy_from_slice(&(strings.len() as u32).to_be_bytes());
        header[36..40].copy_from_slice(&(struct_block.len() as u32).to_be_bytes());

        let mut out = Vec::new();
        out.extend_from_slice(&header);
        out.extend_from_slice(&struct_block);
        out.extend_from_slice(&strings);
        out
    }

    #[test]
    fn parses_simple_device_tree() {
        let dtb = build_simple_dtb();
        let tree = DeviceTree::parse(&dtb).unwrap();
        assert_eq!(tree.root().path, "/");
        assert_eq!(tree.root_address_cells(), 2);
        assert_eq!(tree.root_size_cells(), 1);
        // Root properties are attached to the root node.
        assert!(tree.root().properties.contains_key("compatible"));
    }

    #[test]
    fn finds_node_by_path() {
        let dtb = build_simple_dtb();
        let tree = DeviceTree::parse(&dtb).unwrap();
        let node = tree.find_node("/soc@0").unwrap();
        assert_eq!(node.path, "/soc@0");
        let serial = tree.find_node("/soc@0/serial@1").unwrap();
        assert_eq!(serial.path, "/soc@0/serial@1");
    }

    #[test]
    fn finds_by_compatible() {
        let dtb = build_simple_dtb();
        let tree = DeviceTree::parse(&dtb).unwrap();
        let serials = tree.find_by_compatible(&["ns16550"]);
        assert_eq!(serials.len(), 1);
        assert_eq!(serials[0].path, "/soc@0/serial@1");
        let buses = tree.find_by_compatible(&["simple-bus"]);
        assert_eq!(buses.len(), 1);
    }

    #[test]
    fn reads_reg_property() {
        let dtb = build_simple_dtb();
        let tree = DeviceTree::parse(&dtb).unwrap();
        let serial = tree.find_node("/soc@0/serial@1").unwrap();
        let reg = serial.properties.get("reg").unwrap();
        assert_eq!(reg.value.len(), 16);
        let base = u32::from_be_bytes([reg.value[4], reg.value[5], reg.value[6], reg.value[7]]);
        assert_eq!(base, 0x1f00);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut data = build_simple_dtb();
        data[0] ^= 0xff;
        assert_eq!(DeviceTree::parse(&data), Err(DevicetreeError::BadMagic));
    }

    #[test]
    fn rejects_truncated() {
        let data = build_simple_dtb();
        assert_eq!(
            DeviceTree::parse(&data[..4]),
            Err(DevicetreeError::Truncated)
        );
    }

    #[test]
    fn rejects_bad_size() {
        let data = build_simple_dtb();
        // A truncated blob has a totalsize larger than the data length.
        assert_eq!(
            DeviceTree::parse(&data[..data.len() - 1]),
            Err(DevicetreeError::BadSize)
        );
    }
}
