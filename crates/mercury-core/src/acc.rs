//! Aritmética de punto flotante del acumulador.
//!
//! Fuente: CS 225 A §1.7.2 y Apéndice I ("Details of the accumulator arithmetic"),
//! págs. 56–59. Todo se hace con enteros; no se usan flotantes del anfitrión, para
//! reproducir bit a bit el redondeo de la máquina (forzar un 1 en el dígito
//! menos significativo).
//!
//! Representación: el argumento x es una fracción con signo de 30 bits
//! (−1 ≤ x ≤ 1 − 2⁻²⁹); aquí se guarda como entero escalado por 2²⁹. El
//! acumulador tiene un dígito extra por encima del signo (no accesible al
//! programador), así que en él `frac` puede valer en [−2³⁰, 2³⁰).
//! El exponente y es de 10 bits; un dígito se reserva para detectar desborde,
//! de modo que el rango válido es −256 ≤ y ≤ 255.

use crate::store::sext;

/// 1.0 en la escala del argumento.
pub const ONE: i64 = 1 << 29;
/// 0.5 en la escala del argumento.
pub const HALF: i64 = 1 << 28;
const MASK30: i64 = (1 << 30) - 1;
const MASK29: i64 = (1 << 29) - 1;

pub const EXP_MIN: i32 = -256;
pub const EXP_MAX: i32 = 255;

/// Contenido del acumulador (o de un número largo ya decodificado).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Float {
    /// Argumento escalado por 2²⁹.
    pub frac: i64,
    /// Exponente.
    pub exp: i32,
}

/// El resultado de una operación superó el exponente máximo: la máquina se detiene
/// (§1.7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overflow;

impl Float {
    /// Decodifica un registro largo [y, x_bajo, x_medio, x_alto].
    pub fn from_long(l: [u16; 4]) -> Self {
        let raw = ((l[3] as i64) << 20) | ((l[2] as i64) << 10) | l[1] as i64;
        Float { frac: sext(raw & MASK30, 30), exp: sext(l[0] as i64 & 0x3ff, 10) as i32 }
    }

    /// Codifica a registro largo. Copia exacta (instrucción 41): el dígito extra
    /// del acumulador se pierde.
    pub fn to_long(self) -> [u16; 4] {
        let f = self.frac & MASK30;
        [
            (self.exp as i64 & 0x3ff) as u16,
            (f & 0x3ff) as u16,
            ((f >> 10) & 0x3ff) as u16,
            ((f >> 20) & 0x3ff) as u16,
        ]
    }

    /// Exponente como 10 bits (para las instrucciones 70–77 y 90–97).
    pub fn exp_bits(self) -> u16 {
        (self.exp & 0x3ff) as u16
    }

    /// ¿Está estandarizado? (los dos dígitos más significativos difieren)
    pub fn is_standard(self) -> bool {
        !(-HALF..HALF).contains(&self.frac) && (-ONE..ONE).contains(&self.frac)
    }

    /// Valor aproximado en `f64` (sólo para mostrar; nunca para calcular).
    pub fn to_f64(self) -> f64 {
        (self.frac as f64 / ONE as f64) * 2f64.powi(self.exp)
    }

    /// Construye un número estandarizado a partir de un `f64` (sólo para
    /// preparar datos o programas de prueba; no interviene en la emulación).
    pub fn from_f64(v: f64) -> Self {
        if v == 0.0 {
            return Float { frac: 0, exp: EXP_MIN };
        }
        let mut exp = v.abs().log2().floor() as i32 + 1;
        let mut frac = (v / 2f64.powi(exp) * ONE as f64).round() as i64;
        // Ajustar por redondeo o casos límite (−0.5 debe expresarse como −1·2^(e−1)).
        loop {
            if frac >= ONE || frac < -ONE {
                frac >>= 1;
                exp += 1;
            } else if (-HALF..HALF).contains(&frac) {
                frac <<= 1;
                exp -= 1;
            } else {
                break;
            }
        }
        Float { frac, exp }
    }
}

/// Estandariza: si el argumento desborda al dígito extra se corre una posición
/// hacia abajo (redondeando de nuevo si corresponde); si los dos dígitos más
/// significativos son iguales se corre hacia arriba entrando ceros por debajo.
fn standardise(mut f: Float, round: bool) -> Float {
    if f.frac >= ONE || f.frac < -ONE {
        f.frac >>= 1;
        f.exp += 1;
        if round {
            f.frac |= 1;
        }
    } else if f.frac == 0 {
        // Un cero nunca llega a estandarizarse: el exponente cae hasta el mínimo.
        f.exp = EXP_MIN;
    } else {
        while (-HALF..HALF).contains(&f.frac) {
            f.frac <<= 1;
            f.exp -= 1;
        }
    }
    f
}

/// Comprueba el exponente del resultado (§1.7.2): si y > 255 la máquina se
/// detiene; si y < −256 se lleva a −256 sin tocar x.
fn check_exp(mut f: Float) -> Result<Float, Overflow> {
    if f.exp > EXP_MAX {
        return Err(Overflow);
    }
    if f.exp < EXP_MIN {
        f.exp = EXP_MIN;
    }
    Ok(f)
}

/// Variante de suma/resta (instrucciones 42–47).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddMode {
    /// 42/43: redondeada y estandarizada.
    Rounded,
    /// 44/45: estandarizada sin redondeo.
    Unrounded,
    /// 46/47: sin redondeo ni estandarización; el resultado se corre una
    /// posición hacia abajo y el exponente sube en uno.
    Unstandardised,
}

/// Suma (o resta, si `subtract`) un número largo al acumulador.
/// Devuelve el nuevo acumulador y la diferencia de exponentes (registro "shift",
/// que consulta la instrucción 48).
pub fn add(a: Float, l: Float, subtract: bool, mode: AddMode) -> Result<(Float, u32), Overflow> {
    let mut xl = l.frac;
    if subtract {
        xl = -xl;
    }
    let mut xa = a.frac;
    let d = a.exp - l.exp;
    let shift = d.unsigned_abs();

    if shift >= 32 {
        // Apéndice I, nota: no se suma y se toma el número mayor como respuesta.
        // Supuesto: se entrega tal cual, salvo corregir el desborde que puede
        // producir negar −1 (sólo en las variantes estandarizadas).
        let big = if d > 0 { Float { frac: xa, exp: a.exp } } else { Float { frac: xl, exp: l.exp } };
        let r = match mode {
            AddMode::Unstandardised => big,
            _ => standardise(big, false),
        };
        return check_exp(r).map(|r| (r, shift));
    }

    let exp = if d >= 0 {
        xl >>= shift;
        a.exp
    } else {
        xa >>= shift;
        l.exp
    };
    let mut r = Float { frac: xa + xl, exp };

    match mode {
        AddMode::Rounded => {
            r.frac |= 1;
            r = standardise(r, true);
        }
        AddMode::Unrounded => r = standardise(r, false),
        AddMode::Unstandardised => {
            r.frac >>= 1;
            r.exp += 1;
        }
    }
    check_exp(r).map(|r| (r, shift))
}

/// Variante de multiplicación (instrucciones 50–55).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MulMode {
    /// 50/51: mitad más significativa, redondeada y estandarizada.
    Rounded,
    /// 52/53: mitad más significativa, estandarizada sin redondeo.
    Unrounded,
    /// 54/55: 29 dígitos menos significativos, positivo, exponente − 29.
    LeastSignificant,
}

/// Multiplica el acumulador por un número largo (`negate` para 51, 53 y 55).
pub fn mul(a: Float, l: Float, negate: bool, mode: MulMode) -> Result<Float, Overflow> {
    // Producto de dos fracciones de 30 dígitos: 60 dígitos, punto binario entre
    // los dígitos 58 y 57 (Apéndice I, fig. 2).
    let mut p = a.frac as i128 * l.frac as i128;
    if negate {
        p = -p;
    }
    let exp = a.exp + l.exp;
    let r = match mode {
        MulMode::Rounded => {
            let m = Float { frac: ((p >> 29) as i64) | 1, exp };
            standardise(m, true)
        }
        MulMode::Unrounded => standardise(Float { frac: (p >> 29) as i64, exp }, false),
        MulMode::LeastSignificant => Float { frac: (p as i64) & MASK29, exp: exp - 29 },
    };
    check_exp(r)
}

/// Instrucción 78: fuerza el acumulador a forma no estandarizada con exponente
/// n + 1 (equivale a sumar con 46 un número de argumento 0 y exponente n).
pub fn destandardise(a: Float, n: i32) -> Result<Float, Overflow> {
    add(a, Float { frac: 0, exp: n }, false, AddMode::Unstandardised).map(|(r, _)| r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits(s: &str) -> i64 {
        // "0.11110" → fracción escalada; admite signo por complemento: "1.101110".
        let (int, fr) = s.split_once('.').unwrap();
        let mut v: i64 = if int == "1" { -ONE } else { 0 };
        for (i, c) in fr.chars().enumerate() {
            if c == '1' {
                v += ONE >> (i + 1);
            }
        }
        v
    }

    #[test]
    fn appendix_i_example_unrounded_addition() {
        // A = 1 7/8 (xA = 0.11110, yA = 1); L = 3 (xL = 0.11, yL = 2).
        // Resultado: xA' = 0.1001110, yA' = 3 (= 4 7/8).
        let a = Float { frac: bits("0.11110"), exp: 1 };
        let l = Float { frac: bits("0.11"), exp: 2 };
        let (r, shift) = add(a, l, false, AddMode::Unrounded).unwrap();
        assert_eq!(r, Float { frac: bits("0.1001110"), exp: 3 });
        assert_eq!(shift, 1);
        assert_eq!(r.to_f64(), 4.875);
    }

    #[test]
    fn appendix_i_example_negative_multiplication() {
        // 51: A = 2 1/4 (xA = 0.10010, yA = 2), L = 1/2 (x = 0.10, y = 0).
        // Resultado: xA' = 1.01110 ... 010, yA' = 1.
        let a = Float { frac: bits("0.10010"), exp: 2 };
        let l = Float { frac: bits("0.10"), exp: 0 };
        let r = mul(a, l, true, MulMode::Rounded).unwrap();
        assert_eq!(r.exp, 1);
        assert_eq!(r.frac, bits("1.01110") | 0b10);
    }

    #[test]
    fn rounding_forces_last_digit() {
        let one = Float::from_f64(1.0);
        let (r, _) = add(one, one, false, AddMode::Rounded).unwrap();
        // 1 + 1: 0.1 + 0.1 = 1.0 (desborde) → 0.10..01 con exp 2 (redondeado dos veces).
        assert_eq!(r.exp, 2);
        assert_eq!(r.frac, HALF | 1);
    }

    #[test]
    fn exponent_difference_of_32_takes_larger() {
        let big = Float::from_f64(1.0);
        let small = Float { frac: HALF, exp: big.exp - 40 };
        let (r, shift) = add(big, small, false, AddMode::Rounded).unwrap();
        assert_eq!(r, big);
        assert_eq!(shift, 40);
    }

    #[test]
    fn overflow_and_underflow() {
        let huge = Float { frac: HALF, exp: 255 };
        assert_eq!(mul(huge, huge, false, MulMode::Rounded), Err(Overflow));
        let tiny = Float { frac: HALF, exp: -200 };
        let r = mul(tiny, tiny, false, MulMode::Unrounded).unwrap();
        assert_eq!(r.exp, EXP_MIN);
    }

    #[test]
    fn minus_half_standardises_to_minus_one() {
        let r = standardise(Float { frac: -HALF, exp: 3 }, false);
        assert_eq!(r, Float { frac: -ONE, exp: 2 });
        assert!(r.is_standard());
    }

    #[test]
    fn least_significant_half_is_non_negative() {
        let a = Float::from_f64(-0.75);
        let l = Float::from_f64(0.6);
        let r = mul(a, l, false, MulMode::LeastSignificant).unwrap();
        assert!(r.frac >= 0);
        assert_eq!(r.exp, a.exp + l.exp - 29);
    }

    #[test]
    fn long_round_trip() {
        for v in [1.0, -1.0, 0.5, -0.5, 3.14159, -1e-20, 6.02e23] {
            let f = Float::from_f64(v);
            assert_eq!(Float::from_long(f.to_long()), f);
            assert!((f.to_f64() - v).abs() <= v.abs() * 1e-8);
        }
    }

    #[test]
    fn destandardise_sets_exponent_n_plus_1() {
        let a = Float::from_f64(3.0); // 0.11 · 2^2
        let r = destandardise(a, 10).unwrap();
        assert_eq!(r.exp, 11);
        assert_eq!(r.to_f64(), 3.0);
    }
}
