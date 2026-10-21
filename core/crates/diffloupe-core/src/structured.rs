//! 構造化正規化(JSON / XML / CSV・TSV)。Swift `StructuredNormalizer` 相当。

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StructuredFormat {
    Json,
    Xml,
    Csv,
}

impl StructuredFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Xml => "XML",
            Self::Csv => "CSV",
        }
    }
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "json" => Some(Self::Json),
            "xml" => Some(Self::Xml),
            "csv" | "tsv" => Some(Self::Csv),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StructuredComparisonMode {
    #[default]
    Disabled,
    Automatic,
    Json,
    Xml,
    Csv,
}

impl StructuredComparisonMode {
    pub fn raw_value(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Automatic => "automatic",
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Csv => "csv",
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructuredNormalizationResult {
    pub did_normalize: bool,
    pub format: Option<StructuredFormat>,
    pub normalized_text: Option<String>,
    pub notice: Option<String>,
}

impl StructuredNormalizationResult {
    fn none() -> Self {
        Self {
            did_normalize: false,
            format: None,
            normalized_text: None,
            notice: None,
        }
    }
}

pub const MAX_PARSE_BYTES: usize = 8 * 1024 * 1024;

pub fn normalize(
    data: &[u8],
    url: Option<&str>,
    mode: StructuredComparisonMode,
) -> StructuredNormalizationResult {
    if mode == StructuredComparisonMode::Disabled {
        return StructuredNormalizationResult::none();
    }
    let format = match mode {
        StructuredComparisonMode::Automatic => {
            url.and_then(|u| StructuredFormat::from_extension(crate::paths::path_extension(u)))
        }
        StructuredComparisonMode::Json => Some(StructuredFormat::Json),
        StructuredComparisonMode::Xml => Some(StructuredFormat::Xml),
        StructuredComparisonMode::Csv => Some(StructuredFormat::Csv),
        StructuredComparisonMode::Disabled => None,
    };
    let Some(format) = format else {
        return StructuredNormalizationResult::none();
    };
    // UTF-8 BOM 除去(除去後のサイズで上限判定)
    let content = if data.len() >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
        &data[3..]
    } else {
        data
    };
    if content.len() > MAX_PARSE_BYTES {
        return StructuredNormalizationResult {
            did_normalize: false,
            format: Some(format),
            normalized_text: None,
            notice: Some(format!(
                "{}パース上限(8 MiB)を超えているため、元のテキストで比較します",
                format.display_name()
            )),
        };
    }
    match normalize_text(content, format) {
        Ok(text) => StructuredNormalizationResult {
            did_normalize: true,
            format: Some(format),
            normalized_text: Some(text),
            notice: None,
        },
        Err(()) => StructuredNormalizationResult {
            did_normalize: false,
            format: Some(format),
            normalized_text: None,
            notice: Some(format!(
                "{}として解析できませんでした(元のテキストで比較します)",
                format.display_name()
            )),
        },
    }
}

fn normalize_text(data: &[u8], format: StructuredFormat) -> Result<String, ()> {
    match format {
        StructuredFormat::Json => normalize_json(data),
        StructuredFormat::Xml => normalize_xml(data),
        StructuredFormat::Csv => normalize_csv(data),
    }
}

// MARK: - JSON(JSONSerialization 相当のパース + 決定論的描画)

/// JSONSerialization に近い意味で保持する値
enum JsonValue {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    /// (finite f64)
    Double(f64),
    /// 巨大整数(u64 に収まらない): リテラル保持(NSDecimalNumber 相当)
    BigInt(String),
    String(String),
    Array(Vec<JsonValue>),
    Object(BTreeMap<String, JsonValue>),
}

fn normalize_json(data: &[u8]) -> Result<String, ()> {
    let text = std::str::from_utf8(data).map_err(|_| ())?;
    let mut p = JsonParser {
        s: text.as_bytes(),
        i: 0,
    };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i != p.s.len() {
        return Err(());
    }
    Ok(render_json(&v, 0))
}

struct JsonParser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> JsonParser<'a> {
    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn value(&mut self) -> Result<JsonValue, ()> {
        match self.s.get(self.i) {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(JsonValue::String),
            Some(b't') => self.lit("true").map(|_| JsonValue::Bool(true)),
            Some(b'f') => self.lit("false").map(|_| JsonValue::Bool(false)),
            Some(b'n') => self.lit("null").map(|_| JsonValue::Null),
            Some(_) => self.number(),
            None => Err(()),
        }
    }
    fn lit(&mut self, w: &str) -> Result<(), ()> {
        if self.s[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            Ok(())
        } else {
            Err(())
        }
    }
    fn object(&mut self) -> Result<JsonValue, ()> {
        self.i += 1;
        let mut map = BTreeMap::new();
        self.ws();
        if self.s.get(self.i) == Some(&b'}') {
            self.i += 1;
            return Ok(JsonValue::Object(map));
        }
        loop {
            self.ws();
            let k = self.string()?;
            self.ws();
            if self.s.get(self.i) != Some(&b':') {
                return Err(());
            }
            self.i += 1;
            self.ws();
            let v = self.value()?;
            map.insert(k, v); // 重複キーは後勝ち
            self.ws();
            match self.s.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(JsonValue::Object(map));
                }
                _ => return Err(()),
            }
        }
    }
    fn array(&mut self) -> Result<JsonValue, ()> {
        self.i += 1;
        let mut v = Vec::new();
        self.ws();
        if self.s.get(self.i) == Some(&b']') {
            self.i += 1;
            return Ok(JsonValue::Array(v));
        }
        loop {
            self.ws();
            v.push(self.value()?);
            self.ws();
            match self.s.get(self.i) {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(JsonValue::Array(v));
                }
                _ => return Err(()),
            }
        }
    }
    fn string(&mut self) -> Result<String, ()> {
        if self.s.get(self.i) != Some(&b'"') {
            return Err(());
        }
        self.i += 1;
        let mut out = String::new();
        loop {
            let Some(&b) = self.s.get(self.i) else {
                return Err(());
            };
            match b {
                b'"' => {
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.i += 1;
                    let Some(&e) = self.s.get(self.i) else {
                        return Err(());
                    };
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp) {
                                // サロゲートペア
                                if self.s.get(self.i) == Some(&b'\\')
                                    && self.s.get(self.i + 1) == Some(&b'u')
                                {
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    if !(0xDC00..0xE000).contains(&lo) {
                                        return Err(());
                                    }
                                    let c = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                    out.push(char::from_u32(c).ok_or(())?);
                                } else {
                                    return Err(());
                                }
                            } else if (0xDC00..0xE000).contains(&cp) {
                                return Err(());
                            } else {
                                out.push(char::from_u32(cp).ok_or(())?);
                            }
                        }
                        _ => return Err(()),
                    }
                }
                0x00..=0x1F => return Err(()),
                _ => {
                    // UTF-8 マルチバイトをそのまま取り込む
                    let rest = &self.s[self.i..];
                    let ch_len = utf8_len(b);
                    if ch_len == 0 || rest.len() < ch_len {
                        return Err(());
                    }
                    let chunk = std::str::from_utf8(&rest[..ch_len]).map_err(|_| ())?;
                    out.push_str(chunk);
                    self.i += ch_len;
                }
            }
        }
    }
    fn hex4(&mut self) -> Result<u32, ()> {
        if self.i + 4 > self.s.len() {
            return Err(());
        }
        let h = std::str::from_utf8(&self.s[self.i..self.i + 4]).map_err(|_| ())?;
        let v = u32::from_str_radix(h, 16).map_err(|_| ())?;
        self.i += 4;
        Ok(v)
    }
    fn number(&mut self) -> Result<JsonValue, ()> {
        let start = self.i;
        if self.s.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        // int part
        match self.s.get(self.i) {
            Some(b'0') => self.i += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                    self.i += 1;
                }
            }
            _ => return Err(()),
        }
        let mut is_float = false;
        if self.s.get(self.i) == Some(&b'.') {
            is_float = true;
            self.i += 1;
            if !matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                return Err(());
            }
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        if matches!(self.s.get(self.i), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.i += 1;
            if matches!(self.s.get(self.i), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            if !matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                return Err(());
            }
            while matches!(self.s.get(self.i), Some(b'0'..=b'9')) {
                self.i += 1;
            }
        }
        let lit = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| ())?;
        if is_float {
            let v: f64 = lit.parse().map_err(|_| ())?;
            if !v.is_finite() {
                return Err(()); // JSONSerialization は overflow をエラーにする
            }
            Ok(JsonValue::Double(v))
        } else if let Ok(v) = lit.parse::<i64>() {
            Ok(JsonValue::Int(v))
        } else if let Ok(v) = lit.parse::<u64>() {
            Ok(JsonValue::UInt(v))
        } else {
            Ok(JsonValue::BigInt(lit.to_string()))
        }
    }
}

fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else if b >> 3 == 0b11110 {
        4
    } else {
        0
    }
}

fn render_json(v: &JsonValue, depth: usize) -> String {
    let indent = "  ".repeat(depth);
    let item_indent = "  ".repeat(depth + 1);
    match v {
        JsonValue::Object(map) => {
            if map.is_empty() {
                return "{}".to_string();
            }
            let items: Vec<String> = map
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}{} : {}",
                        item_indent,
                        json_string(k),
                        render_json(v, depth + 1)
                    )
                })
                .collect();
            format!("{{\n{}\n{}}}", items.join(",\n"), indent)
        }
        JsonValue::Array(a) => {
            if a.is_empty() {
                return "[]".to_string();
            }
            let items: Vec<String> = a
                .iter()
                .map(|v| format!("{}{}", item_indent, render_json(v, depth + 1)))
                .collect();
            format!("[\n{}\n{}]", items.join(",\n"), indent)
        }
        JsonValue::String(s) => json_string(s),
        JsonValue::Null => "null".to_string(),
        JsonValue::Bool(b) => b.to_string(),
        JsonValue::Int(i) => i.to_string(),
        JsonValue::UInt(u) => u.to_string(),
        JsonValue::BigInt(s) => s.clone(),
        JsonValue::Double(f) => render_floating(*f),
    }
}

/// Swift `String(Double)` 相当。往復可能な最短表現 + .0 除去規則。
pub fn render_floating(v: f64) -> String {
    let text = swift_double_string(v);
    if v == v.round() && text.ends_with(".0") {
        text[..text.len() - 2].to_string()
    } else {
        text
    }
}

/// Swift `String(value)` (Double.description) の再現:
/// 10進形式は |v| ∈ [1e-4, 2^53]、それ以外は指数表記("d.dddde±XX")。
pub fn swift_double_string(v: f64) -> String {
    if v == 0.0 {
        return if v.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    let (digits, exp10) = shortest_digits(v.abs());
    let k = digits.len() as i32; // 有効桁数
    let n = exp10 + 1; // 小数点位置(整数部の桁数)
    let av = v.abs();
    let sign = if v < 0.0 { "-" } else { "" };
    if (1e-4..=9007199254740992.0).contains(&av) {
        if n <= 0 {
            format!("{}0.{}{}", sign, "0".repeat((-n) as usize), digits)
        } else if n >= k {
            format!("{}{}.0", sign, pad_zeros(&digits, n))
        } else {
            let d = digits;
            format!("{}{}.{}", sign, &d[..n as usize], &d[n as usize..])
        }
    } else {
        let d = digits;
        let mantissa = if d.len() > 1 {
            format!("{}.{}", &d[..1], &d[1..])
        } else {
            d
        };
        let e = exp10;
        format!(
            "{}{}e{}{:02}",
            sign,
            mantissa,
            if e < 0 { "-" } else { "+" },
            e.abs()
        )
    }
}

fn pad_zeros(digits: &str, n: i32) -> String {
    let k = digits.len() as i32;
    if n > k {
        format!("{}{}", digits, "0".repeat((n - k) as usize))
    } else {
        digits.to_string()
    }
}

/// Rust の `{:e}`(最短往復指数表記)から (digits, 指数) を取り出す
fn shortest_digits(v: f64) -> (String, i32) {
    let s = format!("{:e}", v); // 例 "9.9e15", "1e-4"
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    (digits, exp)
}

/// JSONSerialization と同じエスケープ(" \ / + 制御文字)
fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '/' => out.push_str("\\/"),
            '\u{8}' => out.push_str("\\b"),
            '\u{C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// MARK: - XML

enum XmlNode {
    Element(String, Vec<(String, String)>, Vec<XmlNode>),
    Text(String),
    Comment(String),
    Pi(String, String),
}

fn normalize_xml(data: &[u8]) -> Result<String, ()> {
    // 行末正規化(CRLF/CR → LF)を XML 仕様どおり前処理する
    let text_bytes: Vec<u8> = {
        let mut v = Vec::with_capacity(data.len());
        let mut i = 0;
        while i < data.len() {
            if data[i] == b'\r' {
                v.push(b'\n');
                if data.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
            } else {
                v.push(data[i]);
            }
            i += 1;
        }
        v
    };
    let mut reader = quick_xml::Reader::from_reader(std::io::Cursor::new(&text_bytes));
    reader.config_mut().trim_text(false);
    let mut event_buf: Vec<u8> = Vec::new();
    let mut document: Vec<XmlNode> = Vec::new();
    type Frame = (String, Vec<(String, String)>, Vec<XmlNode>);
    let mut stack: Vec<Frame> = Vec::new();
    let mut current_text = String::new();
    let mut saw_root = false;
    let mut depth_ended = false;
    let dec = reader.decoder();
    macro_rules! push_node {
        ($n:expr) => {
            if let Some(last) = stack.last_mut() {
                last.2.push($n);
            } else {
                document.push($n);
            }
        };
    }
    macro_rules! flush_text {
        () => {
            if !current_text.is_empty() {
                let t = std::mem::take(&mut current_text);
                push_node!(XmlNode::Text(t));
            }
        };
    }
    loop {
        match reader.read_event_into(&mut event_buf) {
            Err(_) => return Err(()),
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(quick_xml::events::Event::Start(e)) => {
                if stack.is_empty() {
                    if saw_root {
                        return Err(()); // 複数ルート要素
                    }
                    saw_root = true;
                }
                flush_text!();
                let name = dec.decode(e.name().as_ref()).map_err(|_| ())?.into_owned();
                let mut attrs: Vec<(String, String)> = Vec::new();
                for a in e.attributes() {
                    let a = a.map_err(|_| ())?;
                    let key = dec.decode(a.key.as_ref()).map_err(|_| ())?.into_owned();
                    let val = normalize_attr(&a.value, dec)?;
                    attrs.push((key, val));
                }
                attrs.sort_by(|x, y| x.0.cmp(&y.0));
                stack.push((name, attrs, Vec::new()));
                let _ = depth_ended;
            }
            Ok(quick_xml::events::Event::Empty(e)) => {
                if stack.is_empty() {
                    if saw_root {
                        return Err(());
                    }
                    saw_root = true;
                }
                flush_text!();
                let name = dec.decode(e.name().as_ref()).map_err(|_| ())?.into_owned();
                let mut attrs: Vec<(String, String)> = Vec::new();
                for a in e.attributes() {
                    let a = a.map_err(|_| ())?;
                    let key = dec.decode(a.key.as_ref()).map_err(|_| ())?.into_owned();
                    let val = normalize_attr(&a.value, dec)?;
                    attrs.push((key, val));
                }
                attrs.sort_by(|x, y| x.0.cmp(&y.0));
                push_node!(XmlNode::Element(name, attrs, Vec::new()));
            }
            Ok(quick_xml::events::Event::End(_)) => {
                flush_text!();
                let Some((name, attrs, children)) = stack.pop() else {
                    return Err(());
                };
                if stack.is_empty() {
                    depth_ended = true;
                }
                push_node!(XmlNode::Element(name, attrs, children));
            }
            Ok(quick_xml::events::Event::Text(e)) => {
                let raw = dec.decode(e.as_ref()).map_err(|_| ())?;
                let un = quick_xml::escape::unescape(&raw).map_err(|_| ())?;
                current_text.push_str(&un);
            }
            Ok(quick_xml::events::Event::CData(e)) => {
                let raw = dec.decode(e.as_ref()).map_err(|_| ())?;
                current_text.push_str(&raw);
            }
            Ok(quick_xml::events::Event::Comment(e)) => {
                flush_text!();
                let raw = dec.decode(e.as_ref()).map_err(|_| ())?.into_owned();
                push_node!(XmlNode::Comment(raw));
            }
            Ok(quick_xml::events::Event::PI(e)) => {
                flush_text!();
                let raw = dec.decode(e.as_ref()).map_err(|_| ())?.into_owned();
                let (target, data) = match raw.split_once(char::is_whitespace) {
                    Some((t, d)) => (t.to_string(), d.trim_end().to_string()),
                    None => (raw.clone(), String::new()),
                };
                push_node!(XmlNode::Pi(target, data));
            }
            Ok(quick_xml::events::Event::Decl(_)) => {}
            Ok(quick_xml::events::Event::DocType(_)) => {}
            _ => return Err(()),
        }
    }
    if !stack.is_empty() || document.is_empty() {
        return Err(());
    }
    let mut lines = Vec::new();
    for node in &document {
        render_xml(node, 0, &mut lines);
    }
    Ok(lines.join("\n") + "\n")
}

/// XML 属性値正規化: リテラル空白→スペース、その後 entity 参照を解決
fn normalize_attr(raw: &[u8], dec: quick_xml::Decoder) -> Result<String, ()> {
    let mut text = dec.decode(raw).map_err(|_| ())?.into_owned();
    text = text.replace(['\t', '\n'], " ");
    let un = quick_xml::escape::unescape(&text).map_err(|_| ())?;
    Ok(un.into_owned())
}

fn render_xml(node: &XmlNode, depth: usize, lines: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    match node {
        XmlNode::Text(t) => lines.push(format!("{}{}", indent, escape_text(t))),
        XmlNode::Comment(c) => lines.push(format!("{}<!--{}-->", indent, c)),
        XmlNode::Pi(target, data) => {
            let content = if data.is_empty() {
                String::new()
            } else {
                format!(" {}", data)
            };
            lines.push(format!("{}<?{}{}?>", indent, target, content));
        }
        XmlNode::Element(name, attrs, children) => {
            let attr_string: String = attrs
                .iter()
                .map(|(k, v)| format!(" {}=\"{}\"", k, escape_attr(v)))
                .collect();
            let filtered = filtered_children(children);
            if filtered.is_empty() {
                lines.push(format!("{}<{}{}/>", indent, name, attr_string));
                return;
            }
            if filtered.iter().all(|n| matches!(n, XmlNode::Text(_))) {
                let content: String = filtered
                    .iter()
                    .map(|n| match n {
                        XmlNode::Text(t) => escape_text(t),
                        _ => String::new(),
                    })
                    .collect();
                lines.push(format!(
                    "{}<{}{}>{}</{}>",
                    indent, name, attr_string, content, name
                ));
                return;
            }
            lines.push(format!("{}<{}{}>", indent, name, attr_string));
            for c in filtered {
                render_xml(c, depth + 1, lines);
            }
            lines.push(format!("{}</{}>", indent, name));
        }
    }
}

fn filtered_children(children: &[XmlNode]) -> Vec<&XmlNode> {
    let has_element = children.iter().any(|n| matches!(n, XmlNode::Element(..)));
    let has_sig_text = children.iter().any(|n| match n {
        XmlNode::Text(t) => !t.trim().is_empty(),
        _ => false,
    });
    if !has_element || has_sig_text {
        return children.iter().collect();
    }
    children
        .iter()
        .filter(|n| match n {
            XmlNode::Text(t) => !t.trim().is_empty(),
            _ => true,
        })
        .collect()
}

fn escape_text(t: &str) -> String {
    t.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn escape_attr(t: &str) -> String {
    escape_text(t).replace('"', "&quot;")
}

// MARK: - CSV / TSV

fn normalize_csv(data: &[u8]) -> Result<String, ()> {
    let text = std::str::from_utf8(data).map_err(|_| ())?;
    let delimiter = detect_csv_delimiter(text);
    let rows = parse_csv(text, delimiter).map_err(|_| ())?;
    if rows.is_empty() {
        return Err(());
    }
    Ok(render_csv(&rows))
}

pub fn detect_csv_delimiter(text: &str) -> char {
    let candidates = [',', '\t', ';'];
    let mut counts = [0usize; 3];
    let mut in_quotes = false;
    for ch in text.chars() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if !in_quotes {
            if let Some(i) = candidates.iter().position(|&c| c == ch) {
                counts[i] += 1;
            }
        }
        if ch == '\n' {
            break;
        }
    }
    let mut best = ',';
    let mut best_count = -1i64;
    for (i, c) in candidates.iter().enumerate() {
        if counts[i] as i64 > best_count {
            best = *c;
            best_count = counts[i] as i64;
        }
    }
    best
}

/// CSV パースエラー(未終端引用符など)
#[derive(Debug)]
pub struct CsvParseError;
impl std::fmt::Display for CsvParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "CSV parse error")
    }
}
impl std::error::Error for CsvParseError {}

pub fn parse_csv(text: &str, delimiter: char) -> Result<Vec<Vec<String>>, CsvParseError> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut field_was_quoted = false;
    let mut chars = text.chars().peekable();
    macro_rules! close_field {
        () => {{
            if !field_was_quoted {
                // .whitespaces 相当: スペースとタブのみ(CR は残る)
                field = field.trim_matches([' ', '\t']).to_string();
            }
            row.push(std::mem::take(&mut field));
            field_was_quoted = false;
            let _ = field_was_quoted;
        }};
    }
    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(ch);
            }
        } else if ch == '"' {
            field_was_quoted = true;
            in_quotes = true;
        } else if ch == delimiter {
            close_field!();
        } else if ch == '\n' || ch == '\r' {
            // \r も \n と同じ独立行区切り(CRLF は空行を挟む)
            close_field!();
            rows.push(std::mem::take(&mut row));
        } else {
            field.push(ch);
        }
    }
    if in_quotes {
        return Err(CsvParseError);
    }
    if !field.is_empty() || !row.is_empty() {
        close_field!();
        rows.push(row);
    }
    Ok(rows)
}

fn render_csv_cell(cell: &str) -> String {
    if cell.contains('"') || cell.contains(' ') || cell.contains('\t') {
        format!("\"{}\"", cell.replace('"', "\"\""))
    } else {
        cell.to_string()
    }
}

fn render_csv(rows: &[Vec<String>]) -> String {
    let column_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if column_count == 0 {
        return String::new();
    }
    let mut widths = vec![0usize; column_count];
    for row in rows {
        for (col, cell) in row.iter().enumerate().take(column_count) {
            widths[col] = widths[col].max(render_csv_cell(cell).len());
        }
    }
    let mut lines = Vec::with_capacity(rows.len());
    for row in rows {
        let mut cells = Vec::with_capacity(column_count);
        for (col, &width) in widths.iter().enumerate().take(column_count) {
            let cell = row.get(col).map(|s| s.as_str()).unwrap_or("");
            cells.push(format!("{:<width$}", render_csv_cell(cell), width = width));
        }
        lines.push(cells.join(" "));
    }
    lines.join("\n") + "\n"
}

/// JSONSerialization 相当の文字列エスケープ(レポート側からも使う)
pub fn json_escape_string(s: &str) -> String {
    json_string(s)
}
