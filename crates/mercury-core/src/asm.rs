//! Ensamblador de la *forma escrita* de las instrucciones (CS 225 A, cap. 2).
//!
//! Es una reimplementación en el anfitrión de lo que hacía el *Input Routine*
//! al leer la cinta; sirve mientras el Input Routine original no corra en el
//! emulador. Cubre:
//!
//! * instrucciones `FFb dirección` (función decimal de 2 cifras + dígito B),
//!   p. ej. `420 32`, `017 32+`, `426 16.0`, `080 v1`;
//! * direcciones en forma página.línea (`1.2+` = 66+), negativas y con `+`
//!   (mitad derecha de un registro medio);
//! * etiquetas `(1`–`(99` a la derecha y referencias `v1`–`v99`;
//! * números largos `+123.456`, `-98765`, `-123.456,25` (× 10²⁵), en el
//!   siguiente registro largo, insertando `570` si queda un medio impar;
//! * enteros cortos `=n`, `>n`, `≠n` (también `#n`), en medias palabras;
//! * paréntesis que ignoran texto hasta `)` y el registro par `+)`.
//!
//! Extensiones propias (no existían en la Mercury): `@dirección` fija la
//! posición de ensamblado y `!dirección` la dirección de arranque; `;` inicia
//! un comentario hasta fin de línea.
//!
//! Diferencia conocida: los números largos se convierten con redondeo exacto,
//! mientras que el Input Routine original podía errar en 3 o 4 unidades del
//! último dígito binario (§2.3).

use crate::acc::Float;
use crate::opcodes::{self, AddrType};
use crate::store::Store;
use std::collections::HashMap;

/// Programa ensamblado.
#[derive(Debug, Clone, Default)]
pub struct Program {
    /// (dirección de registro corto, valor de 10 bits).
    pub shorts: Vec<(usize, u16)>,
    /// Dirección de arranque (registro medio).
    pub start: u16,
    /// Etiquetas definidas (número → registro medio).
    pub labels: HashMap<u32, u16>,
}

impl Program {
    pub fn load(&self, store: &mut Store) {
        for &(a, v) in &self.shorts {
            store.set_short(a, v);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsmError {
    pub line: usize,
    pub msg: String,
}

impl std::fmt::Display for AsmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "línea {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for AsmError {}

/// Dirección escrita, en medias palabras (un `+` suma 1 a 2·M).
#[derive(Debug, Clone, Copy)]
enum WAddr {
    /// Valor fijo en unidades de medio registro ×2 (para poder llevar el `+`).
    Fixed(i64),
    /// Referencia a etiqueta `vN`, más desplazamiento en medias palabras.
    Label(u32, i64),
}

enum Item {
    Instr { dec: u8, b: u8, addr: WAddr, line: usize },
    Long(Float),
    Short { kind: char, addr: WAddr, line: usize },
}

/// Lee una dirección escrita: `32`, `32+`, `-2`, `1.2+`, `v1`, `v1+`.
fn parse_addr(s: &str) -> Result<WAddr, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(WAddr::Fixed(0));
    }
    let (body, plus) = match s.strip_suffix('+') {
        Some(b) => (b, 1),
        None => (s, 0),
    };
    if let Some(n) = body.strip_prefix('v') {
        let n: u32 = n.parse().map_err(|_| format!("etiqueta inválida: {s}"))?;
        return Ok(WAddr::Label(n, plus));
    }
    let (neg, body) = match body.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, body),
    };
    let m: i64 = if let Some((p, l)) = body.split_once('.') {
        let p: i64 = p.parse().map_err(|_| format!("página inválida: {s}"))?;
        let l: i64 = l.parse().map_err(|_| format!("línea inválida: {s}"))?;
        p * 64 + l
    } else {
        body.parse().map_err(|_| format!("dirección inválida: {s}"))?
    };
    let half = 2 * m + plus;
    Ok(WAddr::Fixed(if neg { -half } else { half }))
}

/// Número largo escrito: `+123.456`, `-98765`, `-123.456,25`.
fn parse_long(s: &str) -> Result<Float, String> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let (mant, exp) = match s.split_once(',') {
        Some((m, e)) => (m.to_string(), e.parse::<i32>().map_err(|_| format!("exponente inválido: {s}"))?),
        None => (s.clone(), 0),
    };
    let v: f64 = mant.parse().map_err(|_| format!("número inválido: {s}"))?;
    Ok(Float::from_f64(v * 10f64.powi(exp)))
}

/// Ensambla un texto en forma escrita. `origin` es el registro medio inicial
/// (por omisión, página 1). Supuesto: la página 0 queda para constantes e
/// indicadores del Input Routine; falta confirmarlo con CS 225 A §7.3.
pub fn assemble(src: &str) -> Result<Program, AsmError> {
    assemble_at(src, 64)
}

pub fn assemble_at(src: &str, origin: u16) -> Result<Program, AsmError> {
    // Primera pasada: ubicar cada ítem y registrar etiquetas.
    let mut placed: Vec<(usize, Item)> = Vec::new(); // (posición en medias palabras, ítem)
    let mut labels: HashMap<u32, u16> = HashMap::new();
    let mut pos: usize = origin as usize * 2;
    let mut start: Option<WAddr> = None;
    let mut in_bracket = false;

    let err = |line: usize, msg: String| AsmError { line, msg };

    for (i, raw) in src.lines().enumerate() {
        let line = i + 1;
        // Los paréntesis se procesan antes que los comentarios ';' (extensión
        // propia), porque el texto entre paréntesis puede contener ';'.
        let mut text = raw.trim().to_string();

        if in_bracket {
            match text.find(')') {
                Some(k) => {
                    in_bracket = false;
                    text = text[k + 1..].trim().to_string();
                }
                None => continue,
            }
        }
        if text.is_empty() {
            continue;
        }
        // Registro par: "+)" antes del ítem.
        if let Some(rest) = text.strip_prefix("+)") {
            pos = pos.div_ceil(2) * 2; // alinear a medio entero
            if pos % 4 != 0 {
                placed.push((pos, Item::Instr { dec: 57, b: 0, addr: WAddr::Fixed(0), line }));
                pos += 2;
            }
            text = rest.trim().to_string();
            if text.is_empty() {
                continue;
            }
        }
        // Paréntesis al principio de la línea: ignorar hasta ')'.
        if let Some(rest) = text.strip_prefix('(') {
            match rest.find(')') {
                Some(k) => {
                    text = rest[k + 1..].trim().to_string();
                    if text.is_empty() {
                        continue;
                    }
                }
                None => {
                    in_bracket = true;
                    continue;
                }
            }
        }
        let text = text.split(';').next().unwrap_or("").trim().to_string();
        if text.is_empty() {
            continue;
        }
        if let Some(a) = text.strip_prefix('@') {
            let a = parse_addr(a).map_err(|m| err(line, m))?;
            match a {
                WAddr::Fixed(h) if h >= 0 => pos = h as usize,
                _ => return Err(err(line, "@ requiere una dirección fija".into())),
            }
            continue;
        }
        if let Some(a) = text.strip_prefix('!') {
            start = Some(parse_addr(a).map_err(|m| err(line, m))?);
            continue;
        }

        // Etiqueta a la derecha: "(N".
        let (body, label) = match text.rfind('(') {
            Some(k) => {
                let n: u32 = text[k + 1..].trim().parse().map_err(|_| err(line, format!("etiqueta inválida: {text}")))?;
                (text[..k].trim().to_string(), Some(n))
            }
            None => (text.clone(), None),
        };

        let first = body.chars().next().unwrap();
        let item_pos;
        let item = match first {
            '+' | '-' => {
                // Número largo en el siguiente registro largo.
                pos = pos.div_ceil(2) * 2;
                if pos % 4 != 0 {
                    placed.push((pos, Item::Instr { dec: 57, b: 0, addr: WAddr::Fixed(0), line }));
                    pos += 2;
                }
                item_pos = pos;
                pos += 4;
                Item::Long(parse_long(&body).map_err(|m| err(line, m))?)
            }
            '=' | '>' | '≠' | '#' => {
                let kind = if first == '#' { '≠' } else { first };
                let rest: String = body.chars().skip(1).collect();
                item_pos = pos;
                pos += 1;
                Item::Short { kind, addr: parse_addr(&rest).map_err(|m| err(line, m))?, line }
            }
            _ => {
                let mut parts = body.split_whitespace();
                let f = parts.next().unwrap();
                if f.len() != 3 || !f.chars().all(|c| c.is_ascii_digit()) {
                    return Err(err(line, format!("se esperaba función de 3 cifras (FFb): {f}")));
                }
                let dec: u8 = f[..2].parse().unwrap();
                let b: u8 = f[2..].parse().unwrap();
                if b > 7 {
                    return Err(err(line, format!("dígito B fuera de rango: {b}")));
                }
                let a = parts.collect::<Vec<_>>().join("");
                pos = pos.div_ceil(2) * 2; // las instrucciones ocupan un medio entero
                item_pos = pos;
                pos += 2;
                Item::Instr { dec, b, addr: parse_addr(&a).map_err(|m| err(line, m))?, line }
            }
        };
        if let Some(n) = label {
            labels.insert(n, (item_pos / 2) as u16);
        }
        placed.push((item_pos, item));
    }

    let resolve = |a: WAddr, line: usize| -> Result<i64, AsmError> {
        match a {
            WAddr::Fixed(h) => Ok(h),
            WAddr::Label(n, plus) => labels
                .get(&n)
                .map(|m| 2 * *m as i64 + plus)
                .ok_or_else(|| err(line, format!("etiqueta v{n} sin definir"))),
        }
    };

    // Segunda pasada: generar las palabras.
    let mut prog = Program { labels: labels.clone(), ..Default::default() };
    for (p, item) in placed {
        match item {
            Item::Instr { dec, b, addr, line } => {
                let h = resolve(addr, line)?;
                let (bin, n) = encode_instr(dec, h).map_err(|m| err(line, m))?;
                let w = opcodes::make_instruction(bin, b, n);
                prog.shorts.push((p, (w >> 10) as u16));
                prog.shorts.push((p + 1, (w & 0x3ff) as u16));
            }
            Item::Long(f) => {
                for (k, s) in f.to_long().iter().enumerate() {
                    prog.shorts.push((p + k, *s));
                }
            }
            Item::Short { kind, addr, line } => {
                let h = resolve(addr, line)?;
                // §2.4: '>' como tipo 2 (doble), '=' como tipo 1, '≠' como tipo 4 (mitad).
                let v = match kind {
                    '>' => h.rem_euclid(1024),
                    '=' => {
                        if h % 2 != 0 {
                            return Err(err(line, "'=' no admite '+'".into()));
                        }
                        (h / 2).rem_euclid(1024)
                    }
                    _ => {
                        if h % 2 != 0 {
                            return Err(err(line, "'≠' no admite '+'".into()));
                        }
                        (h / 4).rem_euclid(1024)
                    }
                };
                prog.shorts.push((p, v as u16));
            }
        }
    }
    prog.start = match start {
        Some(a) => (resolve(a, 0)? / 2).rem_euclid(1024) as u16,
        None => origin,
    };
    Ok(prog)
}

/// Ensambla un sector transcripto con el número de línea explícito, como en
/// los listados del *Input Routine*: cada línea empieza con la línea del
/// sector (`12`, `12+`) seguida del ítem en forma escrita. Las direcciones
/// van tal como están escritas (absolutas, de la página donde corre el
/// sector), así que la ubicación sólo decide la posición dentro del sector.
/// Devuelve los 128 registros cortos del sector.
pub fn assemble_sector(src: &str) -> Result<[u16; 128], AsmError> {
    let mut asm_src = String::new();
    let mut vistos = std::collections::HashSet::new();
    for (i, raw) in src.lines().enumerate() {
        let text = raw.split(';').next().unwrap_or("").trim();
        if text.is_empty() {
            asm_src.push('\n');
            asm_src.push('\n');
            continue;
        }
        let (pos, item) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
        let h = match parse_addr(pos) {
            Ok(WAddr::Fixed(h)) if (0..128).contains(&h) => h,
            _ => return Err(AsmError { line: i + 1, msg: format!("línea de sector inválida: {pos}") }),
        };
        if !vistos.insert(h) {
            return Err(AsmError { line: i + 1, msg: format!("línea {pos} repetida") });
        }
        // Dos líneas de ensamblador por línea de origen, para conservar la numeración.
        asm_src.push_str(&format!("@{pos}\n{}\n", item.trim()));
    }
    let p = assemble_at(&asm_src, 0).map_err(|e| AsmError { line: e.line.div_ceil(2), msg: e.msg })?;
    let mut sector = [0u16; 128];
    for (a, v) in p.shorts {
        if a >= 128 {
            return Err(AsmError { line: 0, msg: format!("ítem fuera del sector (corto {a})") });
        }
        sector[a] = v;
    }
    Ok(sector)
}

/// Convierte función decimal + dirección escrita (en medias palabras) a
/// (función binaria, dirección de máquina), según el tipo (§3.10).
fn encode_instr(dec: u8, h: i64) -> Result<(u8, u16), String> {
    let base = opcodes::encode(dec, false).unwrap_or(opcodes::PREPULSE_STOP_BIN);
    let need_whole = |h: i64| -> Result<i64, String> {
        if h % 2 != 0 {
            Err(format!("la función {dec:02} no admite dirección con '+'"))
        } else {
            Ok(h / 2)
        }
    };
    match opcodes::addr_type(dec) {
        AddrType::Medium | AddrType::Literal => Ok((base, need_whole(h)?.rem_euclid(1024) as u16)),
        AddrType::Long => {
            let m = need_whole(h)?;
            if m % 2 != 0 {
                return Err(format!("dirección larga impar: {m}"));
            }
            Ok((base, (m / 2).rem_euclid(1024) as u16))
        }
        AddrType::ShortFirstQuarter => Ok((base, h.rem_euclid(1024) as u16)),
        AddrType::Short => {
            let s = h.rem_euclid(2048);
            if s >= 1024 {
                let bin = opcodes::encode(dec, true).ok_or("sin versión de segundo cuarto")?;
                Ok((bin, (s - 1024) as u16))
            } else {
                Ok((base, s as u16))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word_at(p: &Program, m: usize) -> u32 {
        let get = |a: usize| p.shorts.iter().rev().find(|(x, _)| *x == a).map(|x| x.1).unwrap_or(0) as u32;
        (get(2 * m) << 10) | get(2 * m + 1)
    }

    #[test]
    fn manual_addresses() {
        // 017 32+ → H32+ (corto 65); 400 32 → largo 16; 590 v1.
        let p = assemble_at("017 32+\n400 32 (1\n590 v1\n", 0).unwrap();
        let w0 = word_at(&p, 0);
        assert_eq!(w0 >> 13, 0o042); // 01
        assert_eq!((w0 >> 10) & 7, 7);
        assert_eq!(w0 & 0x3ff, 65);
        let w1 = word_at(&p, 1);
        assert_eq!(w1 >> 13, 0o041);
        assert_eq!(w1 & 0x3ff, 16);
        let w2 = word_at(&p, 2);
        assert_eq!(w2 >> 13, 0o001);
        assert_eq!(w2 & 0x3ff, 1);
    }

    #[test]
    fn page_line_and_second_quarter() {
        // 1.2+ = 66+ → corto 133. 007 600 → corto 1200 → segundo cuarto, dirección 176.
        let p = assemble_at("024 1.2+\n007 600\n", 0).unwrap();
        assert_eq!(word_at(&p, 0) & 0x3ff, 133);
        let w = word_at(&p, 1);
        assert_eq!(w >> 13, 0o151); // 00* (Apéndice V §5.3)
        assert_eq!(w & 0x3ff, 176);
    }

    #[test]
    fn short_integers_equivalences() {
        // §2.4: =2, >1 y ≠4 son equivalentes; =-1, >-0+, ≠-2 = 1023.
        let p = assemble_at("=2\n>1\n≠4\n=-1\n>-0+\n#-2\n", 0).unwrap();
        let vals: Vec<u16> = p.shorts.iter().map(|x| x.1).collect();
        assert_eq!(vals, vec![2, 2, 2, 1023, 1023, 1023]);
    }

    #[test]
    fn long_number_alignment_inserts_dummy() {
        let p = assemble_at("590 0\n+3\n", 0).unwrap();
        // 590 en medio 0, 570 en medio 1, +3 en largo 1 (medios 2,3).
        assert_eq!(word_at(&p, 1) >> 13, 0o141);
        let mut st = Store::default();
        p.load(&mut st);
        assert_eq!(Float::from_long(st.long(1)).to_f64(), 3.0);
    }

    #[test]
    fn decimal_exponent() {
        let p = assemble_at("+1,12\n", 0).unwrap();
        let mut st = Store::default();
        p.load(&mut st);
        assert_eq!(Float::from_long(st.long(0)).to_f64(), 1e12);
    }

    #[test]
    fn bracket_ignore() {
        let p = assemble_at("(TITULO\nvarias lineas)\n590 0\n", 0).unwrap();
        assert_eq!(word_at(&p, 0) >> 13, 0o001);
    }

    #[test]
    fn semicolon_inside_bracket_does_not_hide_close() {
        let p = assemble_at("(uno; dos\ntres; cuatro)\n590 0 ; salto\n", 0).unwrap();
        assert_eq!(word_at(&p, 0) >> 13, 0o001);
        assert_eq!(p.shorts.len(), 2);
    }
}
