use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct FirmwareSpec {
    pub firmware: String,
    pub packed_structs: Vec<PackedStructSpec>,
    pub commands: Vec<CommandSpec>,
    pub events: Vec<EventSpec>,
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
    pub return_payload: Option<ReturnPayloadSpec>,
    pub doc: Option<CommandDoc>,
    pub params: Vec<ParamSpec>,
    pub payload: Vec<PayloadField>,
}

#[derive(Debug, Serialize)]
pub struct ReturnPayloadSpec {
    pub struct_name: String,
    pub byte_size: Option<usize>,
    pub fields: Vec<StructFieldSpec>,
}

#[derive(Debug, Serialize)]
pub struct EventSpec {
    pub name: String,
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
    pub constraints: Constraints,
}

#[derive(Debug, Serialize, Clone)]
pub struct ValueDoc {
    pub value: u64,
    pub raw: String,
    pub label: Option<String>,
    pub description: Option<String>,
}

#[derive(Default, Debug, Serialize, Clone)]
pub struct Constraints {
    pub ranges: Vec<RangeDoc>,
    pub unit: Option<UnitDoc>,
}

#[derive(Debug, Serialize, Clone)]
pub struct RangeDoc {
    pub min: u64,
    pub max: u64,
    pub raw: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct UnitDoc {
    pub variable: String,
    pub scale: String,
    pub unit: String,
    pub raw: String,
}

#[derive(Debug, Serialize)]
pub struct PayloadField {
    pub name: String,
    pub c_type: Option<String>,
    pub wire: WireType,
    pub len: Option<String>,
    pub resolved: Option<ResolvedPayload>,
    pub doc: Option<ParamDoc>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResolvedPayload {
    Scalar {
        byte_size: usize,
    },
    Bytes {
        count_expr: String,
        element_size: usize,
    },
    Struct {
        name: String,
        byte_size: usize,
    },
    StructArray {
        name: String,
        element_size: usize,
        count_expr: String,
        byte_len_expr: String,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct PackedStructSpec {
    pub name: String,
    pub byte_size: Option<usize>,
    pub fields: Vec<StructFieldSpec>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StructFieldSpec {
    pub name: String,
    pub c_type: String,
    pub wire: WireType,
    pub array_len: Option<String>,
    pub byte_offset: Option<usize>,
    pub byte_size: Option<usize>,
    pub doc: Option<ParamDoc>,
}

#[derive(Clone, Debug, Serialize)]
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
    if c_type.is_some_and(|c_type| c_type.contains('*')) {
        return WireType::Unknown {
            c_type: c_type.map(str::to_owned),
        };
    }

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
