//! Análisis de las líneas de un programa Autocode.
//!
//! Cada instrucción ocupa una línea. Los espacios no significan nada (el
//! manual escribe `sq rt`, `int pt` y los programas `jump11`, `print(p)1,0`),
//! así que se quitan antes de analizar. Se aceptan equivalentes ASCII de los
//! símbolos del teclado de la Mercury: `->` (→), `>=` (≥), `#` o `!=` (≠),
//! `$` (π), `F` o `X` mayúsculas (φ, la letra de las funciones) y `~` (»,
//! asignación sin redondeo).

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct ErrorSintaxis {
    pub linea: usize,
    pub msg: String,
}

impl fmt::Display for ErrorSintaxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "línea {}: {}", self.linea, self.msg)
    }
}

/// Letras de variables (15 grupos) en el orden del manual.
pub const LETRAS_VARIABLES: [char; 15] = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'u', 'v', 'w', 'x', 'y', 'z', 'π'];

pub fn es_letra_variable(c: char) -> bool {
    LETRAS_VARIABLES.contains(&c)
}

/// Letras de índices: i j k l m n o p q r s t.
pub fn es_indice(c: char) -> bool {
    ('i'..='t').contains(&c)
}

/// Sufijo de una variable principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sufijo {
    /// `x3`
    Fijo(i32),
    /// `xi`, `x(i+3)`, `x(i-50)`
    Indice(char, i32),
}

/// Variable: especial (`x`), especial primada (`x'`) o principal (`x3`, `xi`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variable {
    Especial(char),
    Primada(char),
    Principal(char, Sufijo),
}

/// Factor de un producto.
#[derive(Debug, Clone, PartialEq)]
pub enum Factor {
    Numero(f64),
    Var(Variable),
    Indice(char),
}

/// Término: producto de factores, con un divisor optativo.
#[derive(Debug, Clone, PartialEq)]
pub struct Termino {
    pub negativo: bool,
    pub factores: Vec<Factor>,
    pub divisor: Option<Factor>,
}

/// Suma de términos.
pub type Expr = Vec<Termino>;

/// Término de una expresión de índices: producto de índices y enteros.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminoEntero {
    pub negativo: bool,
    pub factores: Vec<Entero>,
}

/// Índice o entero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entero {
    Lit(i32),
    Ind(char),
}

/// Funciones (`φ nombre (argumentos)`).
#[derive(Debug, Clone, PartialEq)]
pub enum Funcion {
    /// sqrt sin cos tan exp log mod intpt frpt sign
    Una(String, Expr),
    /// divide arctan radius
    Dos(String, Expr, Expr),
    /// parity(n)
    Paridad(Vec<TerminoEntero>),
    Polinomio(Expr, Variable, Entero),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relacion {
    MayorIgual,
    Mayor,
    Igual,
    Distinto,
}

/// Operandos de una comparación.
#[derive(Debug, Clone, PartialEq)]
pub enum Comparacion {
    Variables(Factor, Relacion, Factor),
    Enteros(Entero, Relacion, Entero),
}

/// Operando entre paréntesis de una instrucción de máquina.
#[derive(Debug, Clone, PartialEq)]
pub enum OperandoMaquina {
    /// `400 (x)`: dirección de una variable o constante.
    Simbolico(String),
    /// `200, 15.10+`: dirección escrita convencional.
    Convencional(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperacionPar {
    Suma,
    Resta,
    Producto,
    Cociente,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuncionCompleja {
    Raiz,
    Log,
    Exp,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExpresionPar {
    Copia([Factor; 2]),
    Binaria([Factor; 2], OperacionPar, [Factor; 2]),
    Funcion(FuncionCompleja, [Factor; 2]),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Sentencia {
    /// `y = expr`, `y » expr` (`redondeo` = falso para »).
    AsignarVar { destino: Variable, expr: Expr, redondeo: bool, imprimir: bool },
    /// `y = φ f(x)`
    AsignarFuncion { destino: Variable, f: Funcion, redondeo: bool, imprimir: bool },
    /// `i = expr de índices`
    AsignarIndice { destino: char, expr: Vec<TerminoEntero>, imprimir: bool },
    /// `i = φ int pt (x)`
    IndiceParteEntera { destino: char, expr: Expr, imprimir: bool },
    IndiceExtremo { destino: char, maximo: bool, grupo: Variable, desde: Entero, hasta: Entero, imprimir: bool },
    /// `i = p(q)r` (q negativo para `p(-q)r`)
    Ciclo { indice: char, desde: Entero, paso: Entero, paso_negativo: bool, hasta: Entero },
    Repeat,
    Saltar(u32),
    SaltarSi(u32, Comparacion),
    Check(Expr, Expr, Expr, u32),
    /// `jump (n)`
    SaltarIndice(char),
    /// `n) = 3)`
    MarcaFija(char, u32),
    /// `n) = m)`
    MarcaVariable(char, char),
    Leer(LeerDestino),
    Imprimir { expr: Expr, m: Entero, n: Entero },
    Newline,
    Space,
    End,
    Halt,
    Hoot,
    Maquina { funcion: u8, b: u8, operando: OperandoMaquina },
    // Directivas
    Capitulo(u32),
    Programa(u32),
    Close,
    Reservar(char, i32),
    VariablesDe(u32),
    Titulo,
    Psa,
    /// `φ sqrt` antes de `close`: lista de quickies.
    Quickie(String),
    Across(u32, u32),
    Down(u32, u32),
    AcrossPrograma(u32, u32, u32),
    DownPrograma(u32, u32, u32),
    Up,
    Preserve,
    Restore,
    Rmp,
    IntStep(u32),
    Par { destinos: [Variable; 2], expr: ExpresionPar, doble: bool },
    DobleLongitud,
    Matriz { numero: u8, destino: Option<Variable>, args: Vec<Expr> },
    Auxiliar { escribir: bool, direccion: Expr, variable: Variable, cantidad: Entero },
}

#[derive(Debug, Clone, PartialEq)]
pub enum LeerDestino {
    Var(Variable),
    Indice(char),
}

/// Una línea analizada.
#[derive(Debug, Clone, PartialEq)]
pub struct Linea {
    pub numero: usize,
    pub etiqueta: Option<u32>,
    pub sentencia: Option<Sentencia>,
    /// Texto original (para el listado y los títulos).
    pub texto: String,
}

/// Normaliza una línea: transcribe los equivalentes ASCII y quita los espacios.
pub fn normalizar(s: &str) -> String {
    let s = s.replace("->", "→").replace(">=", "≥").replace("!=", "≠");
    let mut out = String::new();
    for c in s.chars() {
        let c = match c {
            '®' => '→',
            '#' => '≠',
            '$' => 'π',
            'Æ' | 'F' | 'X' => 'φ',
            '~' | '≈' => '»',
            c if c.is_whitespace() => continue,
            c => c,
        };
        out.push(c);
    }
    out
}

struct Cursor<'a> {
    c: Vec<char>,
    i: usize,
    linea: usize,
    _src: &'a str,
}

type R<T> = Result<T, ErrorSintaxis>;

impl<'a> Cursor<'a> {
    fn new(s: &'a str, linea: usize) -> Self {
        Cursor { c: s.chars().collect(), i: 0, linea, _src: s }
    }
    fn err<T>(&self, msg: impl Into<String>) -> R<T> {
        Err(ErrorSintaxis { linea: self.linea, msg: msg.into() })
    }
    fn ver(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn ver2(&self) -> Option<char> {
        self.c.get(self.i + 1).copied()
    }
    fn fin(&self) -> bool {
        self.i >= self.c.len()
    }
    fn tomar(&mut self, ch: char) -> bool {
        if self.ver() == Some(ch) {
            self.i += 1;
            true
        } else {
            false
        }
    }
    fn esperar(&mut self, ch: char) -> R<()> {
        if self.tomar(ch) {
            Ok(())
        } else {
            self.err(format!("se esperaba '{ch}'"))
        }
    }
    fn prefijo(&mut self, p: &str) -> bool {
        let pc: Vec<char> = p.chars().collect();
        if self.c[self.i..].starts_with(&pc) {
            self.i += pc.len();
            true
        } else {
            false
        }
    }
    fn resto(&self) -> String {
        self.c[self.i..].iter().collect()
    }
    fn entero_sin_signo(&mut self) -> R<i32> {
        let ini = self.i;
        while self.ver().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        if ini == self.i {
            return self.err("se esperaba un número entero");
        }
        let s: String = self.c[ini..self.i].iter().collect();
        s.parse().or_else(|_| self.err("entero demasiado grande"))
    }
    fn numero(&mut self) -> R<f64> {
        let ini = self.i;
        while self.ver().is_some_and(|c| c.is_ascii_digit() || c == '.') {
            self.i += 1;
        }
        let s: String = self.c[ini..self.i].iter().collect();
        if s.chars().filter(|c| *c == '.').count() > 1 || s == "." {
            return self.err(format!("número mal escrito: {s}"));
        }
        let s = if s.ends_with('.') { format!("{s}0") } else { s };
        let s = if s.starts_with('.') { format!("0{s}") } else { s };
        s.parse().or_else(|_| self.err(format!("número mal escrito: {s}")))
    }

    /// Variable que empieza en la posición actual (una letra de variable).
    fn variable(&mut self) -> R<Variable> {
        let l = self.ver().unwrap();
        self.i += 1;
        if self.tomar('\'') {
            if l == 'π' {
                return self.err("no hay π'");
            }
            return Ok(Variable::Primada(l));
        }
        match self.ver() {
            Some(c) if c.is_ascii_digit() => {
                let n = self.entero_sin_signo()?;
                Ok(Variable::Principal(l, Sufijo::Fijo(n)))
            }
            Some(c) if es_indice(c) => {
                self.i += 1;
                Ok(Variable::Principal(l, Sufijo::Indice(c, 0)))
            }
            Some('(') => {
                self.i += 1;
                let suf = self.sufijo_entre_parentesis()?;
                Ok(Variable::Principal(l, suf))
            }
            _ => Ok(Variable::Especial(l)),
        }
    }

    /// `(i)`, `(i+3)`, `(i-50)`, `(3)`.
    fn sufijo_entre_parentesis(&mut self) -> R<Sufijo> {
        let r = match self.ver() {
            Some(c) if es_indice(c) => {
                self.i += 1;
                let mut d = 0;
                if self.tomar('+') {
                    d = self.entero_sin_signo()?;
                } else if self.tomar('-') {
                    d = -self.entero_sin_signo()?;
                }
                Sufijo::Indice(c, d)
            }
            Some(c) if c.is_ascii_digit() => Sufijo::Fijo(self.entero_sin_signo()?),
            _ => return self.err("sufijo inválido"),
        };
        self.esperar(')')?;
        Ok(r)
    }

    /// Expresión de variables: `±término ± término ...`. Termina al final o
    /// ante `)`, `,` o `?`.
    fn expr(&mut self) -> R<Expr> {
        let mut terminos = Vec::new();
        let mut negativo = false;
        if self.tomar('-') {
            negativo = true;
        } else {
            self.tomar('+');
        }
        loop {
            let t = self.termino(negativo)?;
            terminos.push(t);
            if self.tomar('+') {
                negativo = false;
            } else if self.tomar('-') {
                negativo = true;
            } else {
                break;
            }
        }
        Ok(terminos)
    }

    fn factor(&mut self) -> R<Option<Factor>> {
        match self.ver() {
            Some(c) if c.is_ascii_digit() || c == '.' => Ok(Some(Factor::Numero(self.numero()?))),
            Some(c) if es_letra_variable(c) => Ok(Some(Factor::Var(self.variable()?))),
            Some(c) if es_indice(c) => {
                self.i += 1;
                Ok(Some(Factor::Indice(c)))
            }
            _ => Ok(None),
        }
    }

    fn termino(&mut self, negativo: bool) -> R<Termino> {
        let mut factores = Vec::new();
        while let Some(f) = self.factor()? {
            factores.push(f);
        }
        if factores.is_empty() {
            return self.err(format!("se esperaba un término en «{}»", self.resto()));
        }
        let mut divisor = None;
        if self.tomar('/') {
            match self.factor()? {
                Some(f) => divisor = Some(f),
                None => return self.err("se esperaba un divisor"),
            }
            if self.factor()?.is_some() {
                return self.err("el divisor debe ser un solo factor (poner el resto antes de /)");
            }
        }
        Ok(Termino { negativo, factores, divisor })
    }

    fn entero(&mut self) -> R<Entero> {
        match self.ver() {
            Some(c) if es_indice(c) => {
                self.i += 1;
                Ok(Entero::Ind(c))
            }
            Some(c) if c.is_ascii_digit() => Ok(Entero::Lit(self.entero_sin_signo()?)),
            Some('-') if self.ver2().is_some_and(|c| c.is_ascii_digit()) => {
                self.i += 1;
                Ok(Entero::Lit(-self.entero_sin_signo()?))
            }
            _ => self.err("se esperaba un índice o un entero"),
        }
    }

    fn expr_entera(&mut self) -> R<Vec<TerminoEntero>> {
        let mut terminos = Vec::new();
        let mut negativo = false;
        if self.tomar('-') {
            negativo = true;
        } else {
            self.tomar('+');
        }
        loop {
            let mut factores = Vec::new();
            loop {
                match self.ver() {
                    Some(c) if es_indice(c) => {
                        self.i += 1;
                        factores.push(Entero::Ind(c));
                    }
                    Some(c) if c.is_ascii_digit() => factores.push(Entero::Lit(self.entero_sin_signo()?)),
                    Some(c) if es_letra_variable(c) => {
                        return self.err("una expresión de índices no admite variables")
                    }
                    Some('.') | Some('/') => return self.err("una expresión de índices sólo admite enteros"),
                    _ => break,
                }
            }
            if factores.is_empty() {
                return self.err("se esperaba un índice o un entero");
            }
            terminos.push(TerminoEntero { negativo, factores });
            if self.tomar('+') {
                negativo = false;
            } else if self.tomar('-') {
                negativo = true;
            } else {
                break;
            }
        }
        Ok(terminos)
    }
}

/// ¿La expresión tiene sólo índices y enteros?
fn expr_es_entera(e: &Expr) -> bool {
    e.iter().all(|t| {
        t.divisor.is_none()
            && t.factores.iter().all(|f| match f {
                Factor::Indice(_) => true,
                Factor::Numero(x) => x.fract() == 0.0,
                Factor::Var(_) => false,
            })
    })
}

/// Analiza todo el programa.
pub fn analizar(fuente: &str) -> Result<Vec<Linea>, ErrorSintaxis> {
    let mut lineas = Vec::new();
    let mut titulo_pendiente = false;
    for (k, raw) in fuente.lines().enumerate() {
        let numero = k + 1;
        if titulo_pendiente {
            titulo_pendiente = false;
            lineas.push(Linea { numero, etiqueta: None, sentencia: None, texto: raw.trim().to_string() });
            continue;
        }
        // Comentarios al estilo de los programas de Edimburgo: `//` hasta el fin.
        let sin_coment = raw.split("//").next().unwrap_or("");
        let s = normalizar(sin_coment);
        if s.is_empty() {
            continue;
        }
        let (etiqueta, sentencia) = analizar_linea(&s, numero)?;
        if matches!(sentencia, Some(Sentencia::Titulo)) {
            titulo_pendiente = true;
        }
        lineas.push(Linea { numero, etiqueta, sentencia, texto: raw.trim().to_string() });
    }
    Ok(lineas)
}

fn analizar_linea(s: &str, numero: usize) -> R<(Option<u32>, Option<Sentencia>)> {
    let mut cur = Cursor::new(s, numero);
    // Etiqueta: "12)" al principio.
    let mut etiqueta = None;
    {
        let ini = cur.i;
        if cur.ver().is_some_and(|c| c.is_ascii_digit()) {
            let n = cur.entero_sin_signo()?;
            if cur.tomar(')') {
                if !(1..=127).contains(&n) {
                    return cur.err(format!("etiqueta fuera de rango 1-127: {n}"));
                }
                etiqueta = Some(n as u32);
            } else {
                cur.i = ini;
            }
        }
    }
    if cur.fin() {
        return Ok((etiqueta, None));
    }
    let sent = sentencia(&mut cur)?;
    if !cur.fin() {
        return cur.err(format!("sobra «{}»", cur.resto()));
    }
    Ok((etiqueta, Some(sent)))
}

fn palabra_sola(cur: &mut Cursor, p: &str) -> bool {
    let guardado = cur.i;
    if cur.prefijo(p) && cur.fin() {
        true
    } else {
        cur.i = guardado;
        false
    }
}

fn sentencia(cur: &mut Cursor) -> R<Sentencia> {
    for (p, s) in [
        ("newline", Sentencia::Newline),
        ("space", Sentencia::Space),
        ("repeat", Sentencia::Repeat),
        ("end", Sentencia::End),
        ("halt", Sentencia::Halt),
        ("hoot", Sentencia::Hoot),
        ("close", Sentencia::Close),
        ("psa", Sentencia::Psa),
        ("title", Sentencia::Titulo),
        ("caption", Sentencia::Titulo),
        ("up", Sentencia::Up),
        ("preserve", Sentencia::Preserve),
        ("restore", Sentencia::Restore),
        ("rmp", Sentencia::Rmp),
        ("doublelength", Sentencia::DobleLongitud),
    ] {
        if palabra_sola(cur, p) {
            return Ok(s);
        }
    }
    if cur.prefijo("programme-") {
        let n = cur.entero_sin_signo()? as u32;
        if !(1..=1023).contains(&n) {
            return cur.err("el número de programme tiene que estar entre 1 y 1023");
        }
        return Ok(Sentencia::Programa(n));
    }
    if cur.prefijo("chapter") {
        return Ok(Sentencia::Capitulo(cur.entero_sin_signo()? as u32));
    }
    if cur.prefijo("variables") {
        return Ok(Sentencia::VariablesDe(cur.entero_sin_signo()? as u32));
    }
    let es_down = cur.c[cur.i..].starts_with(&['d', 'o', 'w', 'n']);
    if cur.prefijo("across") || cur.prefijo("down") {
        let m = cur.entero_sin_signo()? as u32;
        cur.esperar('/')?;
        let c = cur.entero_sin_signo()? as u32;
        if cur.tomar('-') {
            let p = cur.entero_sin_signo()? as u32;
            return Ok(if es_down { Sentencia::DownPrograma(m, c, p) } else { Sentencia::AcrossPrograma(m, c, p) });
        }
        return Ok(if es_down { Sentencia::Down(m, c) } else { Sentencia::Across(m, c) });
    }
    if cur.prefijo("jump") {
        if cur.tomar('(') {
            let c = cur.ver();
            match c {
                Some(c) if es_indice(c) => {
                    cur.i += 1;
                    cur.esperar(')')?;
                    return Ok(Sentencia::SaltarIndice(c));
                }
                _ => return cur.err("jump (n) requiere un índice"),
            }
        }
        let n = cur.entero_sin_signo()? as u32;
        if cur.fin() {
            return Ok(Sentencia::Saltar(n));
        }
        cur.esperar(',')?;
        return Ok(Sentencia::SaltarSi(n, comparacion(cur)?));
    }
    if cur.prefijo("check(") {
        let x = cur.expr()?;
        cur.esperar(',')?;
        let y = cur.expr()?;
        cur.esperar(',')?;
        let e = cur.expr()?;
        cur.esperar(',')?;
        let m = cur.entero_sin_signo()? as u32;
        cur.esperar(')')?;
        return Ok(Sentencia::Check(x, y, e, m));
    }
    if cur.prefijo("intstep(") {
        let etiqueta = cur.entero_sin_signo()? as u32;
        cur.esperar(')')?;
        return Ok(Sentencia::IntStep(etiqueta));
    }
    if cur.ver() == Some('(') {
        return asignacion_par(cur);
    }
    let escribir = cur.prefijo("φ7(");
    if escribir || cur.prefijo("φ6(") {
        let direccion = cur.expr()?;
        cur.esperar(')')?;
        let variable = cur.variable()?;
        cur.esperar(',')?;
        let cantidad = cur.entero()?;
        return Ok(Sentencia::Auxiliar { escribir, direccion, variable, cantidad });
    }
    if cur.tomar('φ') {
        if cur.ver().is_some_and(|c| c.is_ascii_digit()) {
            return matriz(cur, None);
        }
        cur.i -= 1;
    }
    if cur.prefijo("read(") {
        let d = match cur.ver() {
            Some(c) if es_indice(c) && cur.ver2() == Some(')') => {
                cur.i += 1;
                LeerDestino::Indice(c)
            }
            Some(c) if es_letra_variable(c) => LeerDestino::Var(cur.variable()?),
            _ => return cur.err("read necesita una variable o un índice"),
        };
        cur.esperar(')')?;
        return Ok(Sentencia::Leer(d));
    }
    if cur.prefijo("print(") {
        let expr = cur.expr()?;
        // Forma de Edimburgo `print (x, m, n)` además de la del manual `print (x) m, n`.
        let (m, n) = if cur.tomar(',') {
            let m = cur.entero()?;
            cur.esperar(',')?;
            let n = cur.entero()?;
            cur.esperar(')')?;
            (m, n)
        } else {
            cur.esperar(')')?;
            let m = cur.entero()?;
            cur.esperar(',')?;
            (m, cur.entero()?)
        };
        return Ok(Sentencia::Imprimir { expr, m, n });
    }
    // Instrucción de máquina: tres cifras y '(' o ','.
    let r: Vec<char> = cur.c[cur.i..].to_vec();
    if r.len() > 3 && r[..3].iter().all(|c| c.is_ascii_digit()) && matches!(r[3], '(' | ',') {
        let funcion = (r[0].to_digit(10).unwrap() * 10 + r[1].to_digit(10).unwrap()) as u8;
        let b = r[2].to_digit(10).unwrap() as u8;
        if b > 7 {
            return cur.err("dígito B mayor que 7");
        }
        cur.i += 3;
        let operando = if cur.tomar('(') {
            let ini = cur.i;
            let Some(k) = cur.c[ini..].iter().rposition(|c| *c == ')') else {
                return cur.err("falta ')'");
            };
            let dentro: String = cur.c[ini..ini + k].iter().collect();
            cur.i = ini + k + 1;
            OperandoMaquina::Simbolico(dentro)
        } else {
            cur.esperar(',')?;
            let r = cur.resto();
            cur.i = cur.c.len();
            OperandoMaquina::Convencional(r)
        };
        return Ok(Sentencia::Maquina { funcion, b, operando });
    }
    // Lista de quickies: "φsqrt". Sólo los ocho nombres del manual.
    if cur.ver() == Some('φ') && !cur.c.contains(&'=') {
        cur.i += 1;
        let r = cur.resto();
        const QUICKIES: [&str; 8] = ["sqrt", "cos", "log", "tan", "radius", "sin", "exp", "arctan"];
        if !QUICKIES.contains(&r.as_str()) {
            return cur.err(format!("φ{r} no es un quickie (sqrt, cos, log, tan, radius, sin, exp, arctan)"));
        }
        cur.i = cur.c.len();
        return Ok(Sentencia::Quickie(r));
    }
    // Reserva de variables: "x→99".
    if cur.ver2() == Some('→') && cur.ver().is_some_and(es_letra_variable) {
        let l = cur.ver().unwrap();
        cur.i += 2;
        return Ok(Sentencia::Reservar(l, cur.entero_sin_signo()?));
    }
    // n) = 3)  /  n) = m)
    if cur.ver().is_some_and(es_indice) && cur.ver2() == Some(')') {
        let n = cur.ver().unwrap();
        cur.i += 2;
        cur.esperar('=')?;
        let r = match cur.ver() {
            Some(c) if es_indice(c) => {
                cur.i += 1;
                Sentencia::MarcaVariable(n, c)
            }
            _ => {
                let k = cur.entero_sin_signo()?;
                Sentencia::MarcaFija(n, k as u32)
            }
        };
        cur.esperar(')')?;
        return Ok(r);
    }
    asignacion(cur)
}

fn comparacion(cur: &mut Cursor) -> R<Comparacion> {
    // α rel β. Un signo menos sólo puede preceder a una constante.
    let lado = |cur: &mut Cursor| -> R<Factor> {
        let neg = cur.tomar('-');
        let f = cur.factor()?.ok_or(()).or_else(|_| cur.err("se esperaba un operando"))?;
        if neg {
            match f {
                Factor::Numero(x) => Ok(Factor::Numero(-x)),
                _ => cur.err("el signo menos sólo puede ir delante de una constante"),
            }
        } else {
            Ok(f)
        }
    };
    let a = lado(cur)?;
    let rel = match cur.ver() {
        Some('≥') => Relacion::MayorIgual,
        Some('>') => Relacion::Mayor,
        Some('=') => Relacion::Igual,
        Some('≠') => Relacion::Distinto,
        _ => return cur.err("se esperaba ≥, >, = o ≠"),
    };
    cur.i += 1;
    let b = lado(cur)?;
    let entero = |f: &Factor| match f {
        Factor::Indice(c) => Some(Entero::Ind(*c)),
        Factor::Numero(x) if x.fract() == 0.0 => Some(Entero::Lit(*x as i32)),
        _ => None,
    };
    let es_ind = |f: &Factor| matches!(f, Factor::Indice(_));
    if es_ind(&a) || es_ind(&b) {
        match (entero(&a), entero(&b)) {
            (Some(x), Some(y)) => Ok(Comparacion::Enteros(x, rel, y)),
            _ => cur.err("no se puede comparar una variable con un índice"),
        }
    } else {
        Ok(Comparacion::Variables(a, rel, b))
    }
}

fn factor_par(cur: &mut Cursor) -> R<Factor> {
    let negativo = cur.tomar('-');
    if !negativo {
        cur.tomar('+');
    }
    match cur.factor()? {
        Some(Factor::Numero(x)) => Ok(Factor::Numero(if negativo { -x } else { x })),
        Some(f @ Factor::Var(_)) if !negativo => Ok(f),
        _ => cur.err("se esperaba una variable o una constante con signo"),
    }
}

fn variable_par(cur: &mut Cursor) -> R<Variable> {
    if !cur.ver().is_some_and(es_letra_variable) {
        return cur.err("se esperaba una variable de destino");
    }
    cur.variable()
}

fn par<T>(cur: &mut Cursor, doble: bool, componente: fn(&mut Cursor) -> R<T>) -> R<[T; 2]> {
    cur.esperar('(')?;
    if doble {
        cur.esperar('(')?;
    }
    let primero = componente(cur)?;
    cur.esperar(',')?;
    let segundo = componente(cur)?;
    cur.esperar(')')?;
    if doble {
        cur.esperar(')')?;
    }
    Ok([primero, segundo])
}

fn asignacion_par(cur: &mut Cursor) -> R<Sentencia> {
    let doble = cur.ver2() == Some('(');
    let destinos = par(cur, doble, variable_par)?;
    cur.esperar('=')?;
    let expr = if !doble && cur.tomar('φ') {
        let funcion = if cur.prefijo("sqrt") { FuncionCompleja::Raiz }
            else if cur.prefijo("log") { FuncionCompleja::Log }
            else if cur.prefijo("exp") { FuncionCompleja::Exp }
            else { return cur.err("se esperaba φ sqrt, φ log o φ exp"); };
        ExpresionPar::Funcion(funcion, par(cur, false, factor_par)?)
    } else if doble && cur.prefijo("1/") {
        ExpresionPar::Binaria([Factor::Numero(1.0), Factor::Numero(0.0)],
            OperacionPar::Cociente, par(cur, true, factor_par)?)
    } else {
        let primero = par(cur, doble, factor_par)?;
        if cur.fin() {
            ExpresionPar::Copia(primero)
        } else {
            let operacion = match cur.ver() {
                Some('+') => OperacionPar::Suma,
                Some('-') => OperacionPar::Resta,
                Some('*') => OperacionPar::Producto,
                Some('/') => OperacionPar::Cociente,
                _ => return cur.err("se esperaba +, -, * o /"),
            };
            cur.i += 1;
            ExpresionPar::Binaria(primero, operacion, par(cur, doble, factor_par)?)
        }
    };
    Ok(Sentencia::Par { destinos, expr, doble })
}

fn asignacion(cur: &mut Cursor) -> R<Sentencia> {
    let imprimir_antes = cur.tomar('?');
    let quitar_pregunta_final = |cur: &mut Cursor| {
        if cur.c.last() == Some(&'?') {
            cur.c.pop();
            true
        } else {
            false
        }
    };
    let imprimir_despues = quitar_pregunta_final(cur);
    let imprimir = imprimir_antes || imprimir_despues;

    match cur.ver() {
        Some(c) if es_indice(c) => {
            cur.i += 1;
            cur.esperar('=')?;
            // ¿Ciclo? i = p(q)r
            let guardado = cur.i;
            if let Ok(desde) = cur.entero() {
                if cur.tomar('(') {
                    let paso_negativo = cur.tomar('-');
                    let paso = cur.entero()?;
                    cur.esperar(')')?;
                    let hasta = cur.entero()?;
                    if imprimir {
                        return cur.err("? no se aplica a un ciclo");
                    }
                    return Ok(Sentencia::Ciclo { indice: c, desde, paso, paso_negativo, hasta });
                }
            }
            cur.i = guardado;
            if cur.prefijo("φintpt(") {
                let e = cur.expr()?;
                cur.esperar(')')?;
                return Ok(Sentencia::IndiceParteEntera { destino: c, expr: e, imprimir });
            }
            let maximo = cur.prefijo("φmax(");
            if maximo || cur.prefijo("φmin(") {
                let grupo = cur.variable()?;
                cur.esperar(',')?;
                let desde = cur.entero()?;
                cur.esperar(',')?;
                let hasta = cur.entero()?;
                cur.esperar(')')?;
                return Ok(Sentencia::IndiceExtremo { destino: c, maximo, grupo, desde, hasta, imprimir });
            }
            if cur.ver() == Some('φ') {
                return cur.err("a un índice se le puede asignar φ int pt, φ max o φ min");
            }
            let e = cur.expr_entera()?;
            Ok(Sentencia::AsignarIndice { destino: c, expr: e, imprimir })
        }
        Some(c) if es_letra_variable(c) => {
            let destino = cur.variable()?;
            let redondeo = if cur.tomar('=') {
                true
            } else if cur.tomar('»') {
                false
            } else {
                return cur.err("se esperaba = o »");
            };
            if cur.tomar('φ') {
                if cur.ver().is_some_and(|c| c.is_ascii_digit()) {
                    if imprimir {
                        return cur.err("? no se aplica a una operación matricial");
                    }
                    return matriz(cur, Some(destino));
                }
                if cur.c[cur.i..].starts_with(&['m', 'a', 'x']) || cur.c[cur.i..].starts_with(&['m', 'i', 'n']) {
                    return cur.err("φ max/min devuelve un índice");
                }
                let f = funcion(cur)?;
                return Ok(Sentencia::AsignarFuncion { destino, f, redondeo, imprimir });
            }
            let expr = cur.expr()?;
            let _ = expr_es_entera;
            Ok(Sentencia::AsignarVar { destino, expr, redondeo, imprimir })
        }
        _ => cur.err(format!("instrucción no reconocida: «{}»", cur.resto())),
    }
}

fn funcion(cur: &mut Cursor) -> R<Funcion> {
    const UNA: [&str; 10] = ["sqrt", "sin", "cos", "tan", "exp", "log", "mod", "intpt", "frpt", "sign"];
    const DOS: [&str; 3] = ["divide", "arctan", "radius"];
    for n in UNA {
        if cur.prefijo(&format!("{n}(")) {
            let e = cur.expr()?;
            cur.esperar(')')?;
            return Ok(Funcion::Una(n.to_string(), e));
        }
    }
    for n in DOS {
        if cur.prefijo(&format!("{n}(")) {
            let a = cur.expr()?;
            cur.esperar(',')?;
            let b = cur.expr()?;
            cur.esperar(')')?;
            return Ok(Funcion::Dos(n.to_string(), a, b));
        }
    }
    if cur.prefijo("parity(") {
        let e = cur.expr_entera()?;
        cur.esperar(')')?;
        return Ok(Funcion::Paridad(e));
    }
    if cur.prefijo("poly(") {
        let x = cur.expr()?;
        cur.esperar(')')?;
        let grupo = cur.variable()?;
        cur.esperar(',')?;
        let n = cur.entero()?;
        return Ok(Funcion::Polinomio(x, grupo, n));
    }
    cur.err(format!("función desconocida: φ{}", cur.resto()))
}

fn matriz(cur: &mut Cursor, destino: Option<Variable>) -> R<Sentencia> {
    let numero = cur.entero_sin_signo()?;
    let cantidad = match numero {
        8 | 26 | 27 => 5,
        9 | 13 | 14 | 23 | 24 => 4,
        11 | 12 | 16 | 19..=22 | 28 => 3,
        10 | 15 | 17 | 18 | 25 => 2,
        _ => return cur.err(format!("φ{numero} no es una operación matricial")),
    };
    if (numero >= 11) != destino.is_some() {
        return cur.err(format!("destino incorrecto para φ{numero}"));
    }
    cur.esperar('(')?;
    let mut args = Vec::new();
    for i in 0..cantidad {
        if i > 0 {
            cur.esperar(',')?;
        }
        args.push(cur.expr()?);
    }
    cur.esperar(')')?;
    Ok(Sentencia::Matriz { numero: numero as u8, destino, args })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn una(s: &str) -> Sentencia {
        analizar_linea(&normalizar(s), 1).unwrap().1.unwrap()
    }

    #[test]
    fn convencion_de_sufijos() {
        // y = 2mn a(m+1) + amn + man: 2·m·n·a(m+1) + a_m·n + m·a_n
        let Sentencia::AsignarVar { expr, .. } = una("y = 2mn a(m+1) + amn + man") else { panic!() };
        assert_eq!(
            expr[0].factores,
            vec![
                Factor::Numero(2.0),
                Factor::Indice('m'),
                Factor::Indice('n'),
                Factor::Var(Variable::Principal('a', Sufijo::Indice('m', 1)))
            ]
        );
        assert_eq!(
            expr[1].factores,
            vec![Factor::Var(Variable::Principal('a', Sufijo::Indice('m', 0))), Factor::Indice('n')]
        );
        assert_eq!(
            expr[2].factores,
            vec![Factor::Indice('m'), Factor::Var(Variable::Principal('a', Sufijo::Indice('n', 0)))]
        );
    }

    #[test]
    fn varias_formas() {
        assert_eq!(
            una("s = 0(1)179"),
            Sentencia::Ciclo { indice: 's', desde: Entero::Lit(0), paso: Entero::Lit(1), paso_negativo: false, hasta: Entero::Lit(179) }
        );
        assert_eq!(
            una("r = 9(-1)0"),
            Sentencia::Ciclo { indice: 'r', desde: Entero::Lit(9), paso: Entero::Lit(1), paso_negativo: true, hasta: Entero::Lit(0) }
        );
        assert!(matches!(una("jump 3, x ≥ y"), Sentencia::SaltarSi(3, Comparacion::Variables(..))));
        assert!(matches!(una("jump11"), Sentencia::Saltar(11)));
        assert!(matches!(una("jump 102,25=n"), Sentencia::SaltarSi(102, Comparacion::Enteros(..))));
        assert!(matches!(una("print(p)1,0"), Sentencia::Imprimir { .. }));
        assert!(matches!(una("print (i,4,0)"), Sentencia::Imprimir { .. }));
        assert!(matches!(una("f -> 180"), Sentencia::Reservar('f', 180)));
        assert!(matches!(una("b = F cos (sh)"), Sentencia::AsignarFuncion { .. }));
        assert!(matches!(una("t=Xintpt(b1)"), Sentencia::IndiceParteEntera { destino: 't', .. }));
        assert!(matches!(una("i = φ int pt(3x-y)"), Sentencia::IndiceParteEntera { .. }));
        assert!(matches!(una("620,27"), Sentencia::Maquina { funcion: 62, b: 0, .. }));
        assert!(matches!(una("410 (u)"), Sentencia::Maquina { funcion: 41, b: 0, .. }));
        assert!(matches!(una("n) = 3)"), Sentencia::MarcaFija('n', 3)));
        assert!(matches!(una("x = zyy - 1 ?"), Sentencia::AsignarVar { imprimir: true, .. }));
        assert!(matches!(una("across 2/3"), Sentencia::Across(2, 3)));
        assert!(matches!(una("down 1/1"), Sentencia::Down(1, 1)));
    }
}
