//! Rutinas de apoyo del Autocode, escritas en código Mercury (forma escrita)
//! y ensambladas con el ensamblador del núcleo.
//!
//! El sistema original no se conserva: estas rutinas son una reconstrucción
//! que respeta el mapa de memoria publicado por García Camarero (AUTOCODE,
//! UBA 1961, Apéndice 1). Ver `mapa` para las direcciones.

use mercury_core::asm::{self, AsmError, Program};

/// Página 0: celdas, rutina de división y constantes de la página 31.
pub const PAGINA0: &str = include_str!("../runtime/pagina0.mer");
/// Impresión de números (`print`, `?`).
pub const IMPRIMIR: &str = include_str!("../runtime/imprimir.mer");
/// Lectura de números de la cinta de datos (`read`).
pub const LEER: &str = include_str!("../runtime/leer.mer");
/// Funciones de biblioteca.
pub const RAIZ: &str = include_str!("../runtime/raiz.mer");
pub const EXP: &str = include_str!("../runtime/exp.mer");
pub const LOG: &str = include_str!("../runtime/log.mer");
pub const TRIG: &str = include_str!("../runtime/trig.mer");
pub const ARCTAN: &str = include_str!("../runtime/arctan.mer");

/// Cambio de capítulo (`across`, `down`, `up`), en la página 1.
pub const CAPITULOS: &str = include_str!("../runtime/capitulos.mer");
pub const PRESERVAR: &str = include_str!("../runtime/preservar.mer");
pub const AUXILIAR: &str = include_str!("../runtime/auxiliar.mer");
pub const RMP: &str = include_str!("../runtime/rmp.mer");
pub const INTSTEP: &str = include_str!("../runtime/intstep.mer");
pub const CARGADOR: &str = include_str!("../runtime/cargador.mer");
pub const COMPLEJO: &str = include_str!("../runtime/complejo.mer");
pub const DOBLE: &str = include_str!("../runtime/doble.mer");
pub const MAT_MEMORIA: &str = include_str!("../runtime/mat_memoria.mer");
pub const MAT_IO: &str = include_str!("../runtime/mat_io.mer");
pub const MAT_LIN: &str = include_str!("../runtime/mat_lin.mer");
pub const MAT_MUL: &str = include_str!("../runtime/mat_mul.mer");
pub const MAT_ELIM: &str = include_str!("../runtime/mat_elim.mer");

/// Rutinas que se leen del tambor a la página 15.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Superposicion {
    Preservar,
    Auxiliar,
    Rmp,
    IntStep,
    Cargador,
    Complejo,
    Doble,
    MatMemoria,
    MatIo,
    MatLin,
    MatMul,
    MatElim,
    MatImprimir,
    MatLeer,
}

impl Superposicion {
    pub const ORIGEN: u16 = 960;

    pub fn sector(self) -> u16 {
        match self {
            Self::Preservar => 4,
            Self::Auxiliar => 3,
            Self::Rmp => 6,
            Self::IntStep => 5,
            Self::Cargador => 2,
            Self::Complejo => 11,
            Self::Doble => 7,
            Self::MatMemoria => 491,
            Self::MatIo => 480,
            Self::MatLin => 482,
            Self::MatMul => 484,
            Self::MatElim => 487,
            Self::MatImprimir => 493,
            Self::MatLeer => 497,
        }
    }

    pub fn origen(self) -> u16 {
        match self {
            Self::Complejo | Self::Doble | Self::MatIo | Self::MatLin | Self::MatMul | Self::MatElim => 512,
            Self::MatMemoria => 832,
            Self::MatImprimir | Self::MatLeer => 256,
            _ => Self::ORIGEN,
        }
    }

    pub fn paginas(self) -> u16 {
        match self {
            Self::Complejo => 2,
            Self::Doble => 4,
            Self::MatMemoria | Self::MatIo | Self::MatLin => 2,
            Self::MatMul => 3,
            Self::MatElim | Self::MatImprimir => 4,
            Self::MatLeer => 3,
            _ => 1,
        }
    }

    pub fn ensamblar(self) -> Result<Program, AsmError> {
        let fuente = match self {
            Self::Preservar => PRESERVAR,
            Self::Auxiliar => AUXILIAR,
            Self::Rmp => RMP,
            Self::IntStep => INTSTEP,
            Self::Cargador => CARGADOR,
            Self::Complejo => COMPLEJO,
            Self::Doble => DOBLE,
            Self::MatMemoria => MAT_MEMORIA,
            Self::MatIo => MAT_IO,
            Self::MatLin => MAT_LIN,
            Self::MatMul => MAT_MUL,
            Self::MatElim => MAT_ELIM,
            Self::MatImprimir => IMPRIMIR,
            Self::MatLeer => LEER,
        };
        asm::assemble_at(fuente, self.origen())
    }
}

/// Ensambla la rutina de cambio de capítulo en la página 1.
pub fn capitulos() -> Result<Program, AsmError> {
    asm::assemble_at(CAPITULOS, 64)
}

/// Ensambla la página 0 (posiciones fijas).
pub fn pagina0() -> Result<Program, AsmError> {
    asm::assemble_at(PAGINA0, 0)
}

/// Rutinas que se enlazan dentro del capítulo que las usa, a la manera de los
/// *quickies*. La etiqueta 1 de cada una es su entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rutina {
    Imprimir,
    Leer,
    /// sqrt (y radius)
    Raiz,
    Exp,
    Log,
    /// sin (etiqueta 1), cos (2) y tan (3)
    Trig,
    Arctan,
}

impl Rutina {
    pub fn fuente(self) -> &'static str {
        match self {
            Rutina::Imprimir => IMPRIMIR,
            Rutina::Leer => LEER,
            Rutina::Raiz => RAIZ,
            Rutina::Exp => EXP,
            Rutina::Log => LOG,
            Rutina::Trig => TRIG,
            Rutina::Arctan => ARCTAN,
        }
    }

    pub fn nombre(self) -> &'static str {
        match self {
            Rutina::Imprimir => "print",
            Rutina::Leer => "read",
            Rutina::Raiz => "sqrt",
            Rutina::Exp => "exp",
            Rutina::Log => "log",
            Rutina::Trig => "sin cos tan",
            Rutina::Arctan => "arctan",
        }
    }

    /// Ensambla la rutina a partir del registro medio `origen`.
    pub fn ensamblar(self, origen: u16) -> Result<Program, AsmError> {
        asm::assemble_at(self.fuente(), origen)
    }
}

/// Mapa de memoria (direcciones de registro corto, medio o largo según el
/// nombre). Supuesto donde el Apéndice 1 no dice nada.
pub mod mapa {
    /// Celda corta con el `m` de `print`.
    pub const CELDA_M: usize = 1;
    /// Celda corta con el `n` de `print`.
    pub const CELDA_N: usize = 2;
    /// Entrada de la rutina de división (registro medio 4).
    pub const DIVISION: u16 = 4;
    /// Lugar de trabajo: largos en los registros medios 32, 34, 36 y 38.
    pub const TRABAJO: [u16; 4] = [16, 17, 18, 19];
    /// Largo de conversión de índices [9, 0, 0, k] (registro medio 40).
    pub const CONV: u16 = 20;
    /// Mitad alta de x del largo de conversión (registro corto).
    pub const CONV_ALTO: usize = 83;
    /// Largo de extracción de la parte entera (registro medio 42).
    pub const EXTR: u16 = 21;
    /// Mitad baja de x del largo de extracción (registro corto).
    pub const EXTR_BAJO: usize = 85;
    /// Primer índice (i) en el registro corto 116 (registro medio 58).
    pub const INDICES: usize = 116;
    /// Primer registro medio del capítulo (página 2).
    pub const CAPITULO_INICIO: u16 = 128;
    /// Último largo de la zona de constantes (página 14, registro 62).
    pub const CAPITULO_FIN_LARGO: u16 = 479;
    /// Primera variable principal (página 16, largo 512).
    pub const PRINCIPALES: u16 = 512;
    pub const N_PRINCIPALES: u16 = 480;
    /// Variables especiales en la página 31: a'..z' (14) y a..z (14).
    pub const ESPECIALES: u16 = 992;
    /// Segundo argumento de divide, arctan y radius (página 1, registro 102).
    pub const ARG2: u16 = 51;
    /// Largos de parámetros de la página 1, fuera de las rutinas fijas.
    pub const PARAMETROS: [u16; 4] = [52, 53, 54, 55];
    /// Parámetros matriciales en la parte libre de la página del cargador.
    pub const MATRIZ_ARGS: [u16; 7] = [505, 506, 507, 508, 509, 510, 511];
    /// Corto de la página 1 reservado para B2 de la subsecuencia de int step.
    pub const VUELTA_SUBSECUENCIA: usize = 188;
    /// Cero con exponente 0 (registro medio 56), para estandarizar.
    pub const CERO_EXP0: u16 = 28;
    pub const PI: u16 = 1020;
    pub const CERO: u16 = 1021;
    pub const MENOS_UNO: u16 = 1022;
}
