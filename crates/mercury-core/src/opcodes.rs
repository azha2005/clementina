//! Tabla de códigos de función de la Mercury.
//!
//! Fuente primaria: Ferranti, *Mercury Programmers' Manual* CS 225 A (abril 1960),
//! Apéndice V, §5.2 (decimal → octal) y §5.3 (octal → decimal), págs. 64–66.
//!
//! La máquina tiene 7 dígitos binarios de función. Los programadores escribían un
//! código decimal de dos cifras (00–99) que el *input routine* traducía al código
//! binario; los números decimales **no** se corresponden con los binarios.
//! En la notación octal del manual los 7 bits se agrupan 1-3-3 (p. ej. 40 → 041).
//!
//! Las funciones de registro corto (tipo 5: 00–07, 20–27, 60, 61) existen en dos
//! versiones: con el bit más significativo en 0 se refieren al primer cuarto del
//! almacén y con 1 al segundo cuarto (las marcadas con `*` en §5.3).

/// Código decimal → código binario de 7 bits (`None` = sin asignar, "100" en la tabla).
pub const DEC_TO_BIN: [Option<u8>; 100] = [
    // 00..09
    Some(0o051), Some(0o042), Some(0o050), Some(0o055), Some(0o054),
    Some(0o053), Some(0o052), Some(0o056), Some(0o007), Some(0o020),
    // 10..19
    Some(0o011), None,        Some(0o010), Some(0o015), Some(0o014),
    Some(0o013), Some(0o012), Some(0o016), Some(0o017), None,
    // 20..29
    Some(0o071), Some(0o062), Some(0o070), Some(0o075), Some(0o074),
    Some(0o073), Some(0o072), Some(0o076), Some(0o027), Some(0o040),
    // 30..39
    Some(0o031), None,        Some(0o030), Some(0o035), Some(0o034),
    Some(0o033), Some(0o032), Some(0o036), Some(0o037), None,
    // 40..49
    Some(0o041), Some(0o043), Some(0o044), Some(0o045), Some(0o046),
    Some(0o047), Some(0o146), Some(0o147), Some(0o061), Some(0o021),
    // 50..59
    Some(0o064), Some(0o065), Some(0o066), Some(0o067), Some(0o026),
    Some(0o006), Some(0o143), Some(0o141), Some(0o060), Some(0o001),
    // 60..69
    Some(0o002), Some(0o022), Some(0o125), Some(0o025), Some(0o063),
    Some(0o164), Some(0o165), Some(0o004), Some(0o024), Some(0o005),
    // 70..79
    Some(0o111), Some(0o023), Some(0o110), Some(0o115), Some(0o114),
    Some(0o113), Some(0o112), Some(0o116), Some(0o144), None,
    // 80..89
    Some(0o126), Some(0o124), Some(0o105), Some(0o104), None,
    None,        Some(0o161), Some(0o160), Some(0o121), Some(0o120),
    // 90..99
    Some(0o131), Some(0o137), Some(0o130), Some(0o135), Some(0o134),
    Some(0o133), Some(0o132), Some(0o136), None,        Some(0o000),
];

/// Bit de función que selecciona el segundo cuarto del almacén en las
/// instrucciones de registro corto.
pub const SECOND_QUARTER_BIT: u8 = 0o100;

/// Parada "prepulsable" 1 000 000: el input routine convierte en ella todo código
/// decimal no descrito (§3.13).
pub const PREPULSE_STOP_BIN: u8 = 0o100;

/// Forma de interpretar la dirección (§3.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrType {
    /// Tipo 1: dirección de registro medio (saltos) o irrelevante.
    Medium,
    /// Tipo 2: registro corto, sólo primer cuarto (instrucción 63).
    ShortFirstQuarter,
    /// Tipo 3: la dirección es el propio entero `n`.
    Literal,
    /// Tipo 4: registro largo (40 bits).
    Long,
    /// Tipo 5: registro corto, con versión de primer y segundo cuarto.
    Short,
}

/// Tipo de dirección de cada código decimal.
pub fn addr_type(dec: u8) -> AddrType {
    match dec {
        0..=7 | 20..=27 | 60 | 61 => AddrType::Short,
        63 => AddrType::ShortFirstQuarter,
        10..=17 | 30..=37 => AddrType::Literal,
        40..=47 | 50..=56 | 64 => AddrType::Long,
        _ => AddrType::Medium,
    }
}

/// ¿El dígito B de esta función modifica la dirección? (Apéndice V, nota 3:
/// las casillas sombreadas no son B-modificables; allí el dígito B nombra el
/// registro B operado o el canal de E/S.)
pub fn is_b_modifiable(dec: u8) -> bool {
    !matches!(dec, 0..=7 | 10..=18 | 70..=77 | 90..=97)
}

/// Resultado de decodificar los 7 bits de función.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoded {
    /// Código decimal (00–99).
    pub dec: u8,
    /// Versión de "segundo cuarto" (marcada con `*` en el manual).
    pub second_quarter: bool,
}

/// Código binario de 7 bits → decimal. `None` para códigos sin descripción en el
/// manual (se tratan como parada prepulsable).
pub fn decode(bin: u8) -> Option<Decoded> {
    let bin = bin & 0x7f;
    for (dec, b) in DEC_TO_BIN.iter().enumerate() {
        if *b == Some(bin) {
            return Some(Decoded { dec: dec as u8, second_quarter: false });
        }
    }
    let low = bin & !SECOND_QUARTER_BIT;
    if bin & SECOND_QUARTER_BIT != 0 {
        for (dec, b) in DEC_TO_BIN.iter().enumerate() {
            if *b == Some(low) && addr_type(dec as u8) == AddrType::Short {
                return Some(Decoded { dec: dec as u8, second_quarter: true });
            }
        }
    }
    None
}

/// Código decimal (+ cuarto) → binario. Para funciones de tipo 5 con
/// `second_quarter = true` se pone el bit más significativo.
pub fn encode(dec: u8, second_quarter: bool) -> Option<u8> {
    let b = (*DEC_TO_BIN.get(dec as usize)?)?;
    if second_quarter {
        if addr_type(dec) != AddrType::Short {
            return None;
        }
        Some(b | SECOND_QUARTER_BIT)
    } else {
        Some(b)
    }
}

/// Arma una palabra de instrucción de 20 bits: función (7) | B (3) | dirección (10).
pub fn make_instruction(bin_fn: u8, b: u8, addr: u16) -> u32 {
    ((bin_fn as u32 & 0x7f) << 13) | ((b as u32 & 7) << 10) | (addr as u32 & 0x3ff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_example_40_is_041() {
        assert_eq!(encode(40, false), Some(0o041));
    }

    #[test]
    fn starred_codes_from_table_5_3() {
        // 102 60*, 122 61*, 142 01*, 150..156, 162 21*, 170..176
        let cases = [
            (0o102, 60), (0o122, 61), (0o142, 1), (0o150, 2), (0o151, 0), (0o152, 6),
            (0o153, 5), (0o154, 4), (0o155, 3), (0o156, 7), (0o162, 21), (0o170, 22),
            (0o171, 20), (0o172, 26), (0o173, 25), (0o174, 24), (0o175, 23), (0o176, 27),
        ];
        for (bin, dec) in cases {
            assert_eq!(decode(bin), Some(Decoded { dec, second_quarter: true }), "{bin:o}");
        }
    }

    #[test]
    fn unstarred_round_trip() {
        for dec in 0..100u8 {
            if let Some(bin) = encode(dec, false) {
                assert_eq!(decode(bin), Some(Decoded { dec, second_quarter: false }));
            }
        }
    }

    #[test]
    fn table_is_injective() {
        let mut seen = [false; 128];
        for b in DEC_TO_BIN.iter().flatten() {
            assert!(!seen[*b as usize], "código repetido {b:o}");
            seen[*b as usize] = true;
        }
    }

    #[test]
    fn blank_codes_are_undecoded() {
        assert_eq!(decode(0o003), None);
        assert_eq!(decode(0o100), None);
    }
}
