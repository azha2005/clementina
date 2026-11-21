//! Código de cinta de papel de 5 canales (CS 225 A, Apéndice VI).
//!
//! Cada carácter tiene dos significados según el último cambio de registro:
//! cifras (FIGS, valor 0) o letras (LETS, valor 27). El 31 (todas las
//! perforaciones) es el borrado ER y se ignora.

pub mod tele_input;

pub const FIG_SHIFT: u16 = 0;
pub const LET_SHIFT: u16 = 27;
pub const ERASE: u16 = 31;
pub const CR: u16 = 30;
pub const LF: u16 = 13;
pub const SP: u16 = 14;

/// Significado en registro de cifras (índice = valor). `None` para los códigos
/// de control (cambios de registro, CR, LF, ER).
pub const FIGURES: [Option<char>; 32] = [
    None, Some('1'), Some('2'), Some('*'), Some('4'), Some('('), Some(')'), Some('7'),
    Some('8'), Some('≠'), Some('='), Some('-'), Some('v'), None, Some(' '), Some(','),
    Some('0'), Some('>'), Some('≥'), Some('3'), Some('→'), Some('5'), Some('6'), Some('/'),
    Some('×'), Some('9'), Some('+'), None, Some('.'), Some('n'), None, None,
];

/// Significado en registro de letras.
pub const LETTERS: [Option<char>; 32] = [
    None, Some('A'), Some('B'), Some('C'), Some('D'), Some('E'), Some('F'), Some('G'),
    Some('H'), Some('I'), Some('J'), Some('K'), Some('L'), Some('M'), Some('N'), Some('O'),
    Some('P'), Some('Q'), Some('R'), Some('S'), Some('T'), Some('U'), Some('V'), Some('W'),
    Some('X'), Some('Y'), Some('Z'), None, Some('.'), Some('?'), Some('£'), None,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shift {
    #[default]
    Figures,
    Letters,
}

/// Decodifica una secuencia de caracteres de cinta como lo imprimiría el
/// teleimpresor fuera de línea. Empieza en registro de cifras.
pub fn decode(chars: &[u16]) -> String {
    let mut out = String::new();
    let mut shift = Shift::Figures;
    for &c in chars {
        match c & 0x1f {
            FIG_SHIFT => shift = Shift::Figures,
            LET_SHIFT => shift = Shift::Letters,
            ERASE => {}
            CR => {}
            LF if shift == Shift::Figures => out.push('\n'),
            v => {
                let table = if shift == Shift::Figures { &FIGURES } else { &LETTERS };
                if let Some(ch) = table[v as usize] {
                    out.push(ch);
                }
            }
        }
    }
    out
}

/// Codifica texto en caracteres de cinta, insertando los cambios de registro
/// necesarios. Los saltos de línea se perforan como CR LF. Acepta también los
/// equivalentes ASCII `x` (×) y `#` (≠). Devuelve error con el primer carácter
/// que no existe en el código.
pub fn encode(text: &str) -> Result<Vec<u16>, char> {
    let mut out = vec![FIG_SHIFT];
    let mut shift = Shift::Figures;
    for ch in text.chars() {
        let ch = match ch {
            '#' => '≠',
            '\r' => continue,
            c => c,
        };
        if ch == '\n' {
            if shift != Shift::Figures {
                out.push(FIG_SHIFT);
                shift = Shift::Figures;
            }
            out.push(CR);
            out.push(LF);
            continue;
        }
        let up = ch.to_ascii_uppercase();
        let fig = FIGURES.iter().position(|c| *c == Some(ch));
        let let_ = LETTERS.iter().position(|c| *c == Some(up));
        // Preferir el registro actual si el carácter existe en ambos ('.').
        let (want, v) = match (fig, let_) {
            (Some(f), Some(l)) => {
                if shift == Shift::Figures {
                    (Shift::Figures, f)
                } else {
                    (Shift::Letters, l)
                }
            }
            (Some(f), None) => (Shift::Figures, f),
            (None, Some(l)) => (Shift::Letters, l),
            (None, None) => {
                if ch == 'x' {
                    (Shift::Figures, 24)
                } else {
                    return Err(ch);
                }
            }
        };
        if want != shift {
            out.push(if want == Shift::Figures { FIG_SHIFT } else { LET_SHIFT });
            shift = want;
        }
        out.push(v as u16);
    }
    Ok(out)
}

/// Código ITA2 (Baudot-Murray) de los teletipos de 5 canales, por si una cinta
/// se perforó para un teletipo y no para el teleimpresor Ferranti.
/// Valor = canal 1 (bit 1) … canal 5 (bit 16); 27 = cifras, 31 = letras.
pub mod ita2 {
    pub const FIGS: u16 = 27;
    pub const LTRS: u16 = 31;
    pub const CR: u16 = 8;
    pub const LF: u16 = 2;

    pub const LETTERS: [Option<char>; 32] = [
        None, Some('E'), None, Some('A'), Some(' '), Some('S'), Some('I'), Some('U'),
        None, Some('D'), Some('R'), Some('J'), Some('N'), Some('F'), Some('C'), Some('K'),
        Some('T'), Some('Z'), Some('L'), Some('W'), Some('H'), Some('Y'), Some('P'), Some('Q'),
        Some('O'), Some('B'), Some('G'), None, Some('M'), Some('X'), Some('V'), None,
    ];
    pub const FIGURES: [Option<char>; 32] = [
        None, Some('3'), None, Some('-'), Some(' '), Some('\''), Some('8'), Some('7'),
        None, Some('¿'), Some('4'), Some('¡'), Some(','), Some('!'), Some(':'), Some('('),
        Some('5'), Some('+'), Some(')'), Some('2'), Some('#'), Some('6'), Some('0'), Some('1'),
        Some('9'), Some('?'), Some('&'), None, Some('.'), Some('/'), Some(';'), None,
    ];

    /// Decodifica empezando en registro de letras. CR se ignora y LF es salto de línea.
    pub fn decode(chars: &[u16]) -> String {
        let mut out = String::new();
        let mut figs = false;
        for &c in chars {
            match c & 0x1f {
                FIGS => figs = true,
                LTRS => figs = false,
                CR => {}
                LF => out.push('\n'),
                v => {
                    let t = if figs { &FIGURES } else { &LETTERS };
                    if let Some(ch) = t[v as usize] {
                        out.push(ch);
                    }
                }
            }
        }
        out
    }
}

/// Opciones para dibujar una cinta.
#[derive(Debug, Clone, Default)]
pub struct SvgOptions {
    /// Filas de guía (sólo arrastre) antes y después del contenido.
    pub leader: usize,
    /// Defectos de avance: `(fila, fracción)` hace que la fila `fila` (contada
    /// desde 0 en el contenido) quede a `fracción` de paso de la anterior en
    /// lugar de a un paso entero, como en las cintas del museo.
    pub overlaps: Vec<(usize, f64)>,
}

/// Dibuja una cinta de 5 canales en SVG **a escala real**: 11/16" (17,46 mm) de
/// ancho, 0,1" (2,54 mm) entre filas y entre canales, agujeros de datos de
/// 0,072" y de arrastre de 0,046". De arriba hacia abajo: canales 16, 8 y 4,
/// arrastre, canales 2 y 1. Impresa al 100 % coincide con una cinta verdadera.
pub fn to_svg(chars: &[u16], leader: usize) -> String {
    to_svg_with(chars, &SvgOptions { leader, ..Default::default() })
}

pub fn to_svg_with(chars: &[u16], opt: &SvgOptions) -> String {
    const PITCH: f64 = 2.54;
    const WIDTH: f64 = 17.46;
    const MARGIN: f64 = (WIDTH - 5.0 * PITCH) / 2.0;
    const R_DATA: f64 = 0.072 * 25.4 / 2.0;
    const R_FEED: f64 = 0.046 * 25.4 / 2.0;
    let y = |k: usize| MARGIN + k as f64 * PITCH;
    let rows: [(u16, usize); 5] = [(16, 0), (8, 1), (4, 2), (2, 4), (1, 5)];

    // Posición de cada fila a lo largo de la cinta.
    let n = chars.len() + 2 * opt.leader;
    let mut xs = Vec::with_capacity(n);
    let mut x = 2.0 * PITCH;
    for i in 0..n {
        if i > 0 {
            let step = i
                .checked_sub(opt.leader)
                .and_then(|c| opt.overlaps.iter().find(|(f, _)| *f == c))
                .map(|(_, frac)| *frac)
                .unwrap_or(1.0);
            x += step * PITCH;
        }
        xs.push(x);
    }
    let len = x + 2.0 * PITCH;

    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {len:.2} {WIDTH}\"          width=\"{len:.2}mm\" height=\"{WIDTH}mm\">
         <rect width=\"{len:.2}\" height=\"{WIDTH}\" fill=\"#efe2b8\"/>
"
    );
    for (i, &x) in xs.iter().enumerate() {
        s += &format!("<circle cx=\"{x:.3}\" cy=\"{:.3}\" r=\"{R_FEED:.3}\" fill=\"#222\"/>
", y(3));
        if i >= opt.leader && i < opt.leader + chars.len() {
            let v = chars[i - opt.leader] & 0x1f;
            for (bit, k) in rows {
                if v & bit != 0 {
                    s += &format!("<circle cx=\"{x:.3}\" cy=\"{:.3}\" r=\"{R_DATA:.3}\" fill=\"#222\"/>
", y(k));
                }
            }
        }
    }
    s += "</svg>
";
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn ita2_museum_tape() {
        let v = [8, 2, 31, 14, 3, 18, 14, 7, 18, 3, 12, 9, 24, 4, 22, 10, 6, 28, 24, 5, 27, 14, 8, 2];
        assert_eq!(super::ita2::decode(&v), "\nCALCULANDO PRIMOS:\n");
    }

    use super::*;

    #[test]
    fn cinta_del_documental() {
        // Filas de La Ciudad Universitaria, del mismo plano que aparece en
        // el capítulo 1 de Clementina de TEC. Ver docs/cinta/VIDEO_TEC.md.
        let v = [
            0, 14, 14, 11, 16, 28, 16, 8, 2, 16, 8, 14, 14, 0, 30, 13, 30,
            0, 14, 16, 28, 8, 7, 14, 14, 0, 14, 14, 11, 16, 28, 16, 8, 7, 16, 19, 14, 14, 0, 30, 13, 30,
            0, 14, 16, 28, 8, 8, 14, 14, 0, 14,
        ];
        assert_eq!(decode(&v), "  -0.08208  \n 0.87    -0.08703  \n 0.88   ");
    }

    #[test]
    fn round_trip() {
        let text = "CLEMENTINA 1961\nX = 3.25 + 4\n";
        let tape = encode(text).unwrap();
        assert_eq!(decode(&tape), text);
    }

    #[test]
    fn space_value_is_14() {
        // §3.9: con Sac = 6, "911 8" envía el espacio, de valor 14.
        assert_eq!(SP, 6 + 8);
        assert_eq!(FIGURES[SP as usize], Some(' '));
    }
}
