use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct FirmwareSpec {
    pub firmware: String,
    pub packed_structs: Vec<PackedStructSpec>,
    pub commands: Vec<CommandSpec>,
}

#[derive(Debug, Serialize)]
pub struct CommandSpec {
    pub group: String,
    pub name: String,
    pub ogf: Option<u16>,
    pub ocf: Option<u16>,
    pub opcode: Option<u16>,
    pub event: Option<u8>,
    pub return_len: Option<usize>,
    pub doc: Option<CommandDoc>,
    pub params: Vec<ParamSpec>,
    pub payload: Vec<PayloadField>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CommandDoc {
    pub brief: Option<String>,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct ParamSpec {
    pub name: String,
    pub c_type: String,
    pub doc: Option<ParamDoc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ParamDoc {
    pub description: String,
    pub values: Vec<ValueDoc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ValueDoc {
    pub value: u64,
    pub raw: String,
    pub label: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PayloadField {
    pub name: String,
    pub c_type: Option<String>,
    pub wire: WireType,
    pub len: Option<String>,
    pub doc: Option<ParamDoc>,
}

#[derive(Debug, Serialize)]
pub struct PackedStructSpec {
    pub name: String,
    pub fields: Vec<StructFieldSpec>,
}

#[derive(Debug, Serialize)]
pub struct StructFieldSpec {
    pub name: String,
    pub c_type: String,
    pub wire: WireType,
    pub array_len: Option<String>,
    pub doc: Option<ParamDoc>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WireType {
    U8,
    U16Le,
    U32Le,
    Bytes,
    Struct { name: String },
    Unknown { c_type: Option<String> },
}

pub fn wire_type_for(c_type: Option<&str>) -> WireType {
    match c_type.map(normalize_c_type).as_deref() {
        Some("uint8_t") => WireType::U8,
        Some("uint16_t") => WireType::U16Le,
        Some("uint32_t") => WireType::U32Le,
        Some(t) if t.ends_with("_t") => WireType::Struct { name: t.into() },
        Some(_) | None => WireType::Unknown {
            c_type: c_type.map(str::to_owned),
        },
    }
}

pub fn normalize_c_type(c_type: &str) -> String {
    c_type
        .replace("const", "")
        .replace('*', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
