//! Programas Autocode compilados y corridos en el emulador.

use mercury_autocode::compilar;
use mercury_core::acc::Float;
use mercury_core::io::BufferIo;
use mercury_core::tape;
use mercury_core::{Machine, Stop};

struct Corrida {
    m: Machine,
    salida: String,
}

fn correr_con(fuente: &str, datos: &str) -> Corrida {
    let img = compilar(fuente).unwrap_or_else(|e| panic!("{e}"));
    let mut m = Machine::new(Default::default());
    img.cargar(&mut m);
    let cinta = if datos.is_empty() { Vec::new() } else { tape::encode(datos).unwrap() };
    let mut io = BufferIo::with_tape(&cinta);
    match m.run(&mut io, 50_000_000) {
        Some(Stop::Stop99) => {}
        otro => panic!("terminó con {otro:?}\n{}", img.listado),
    }
    Corrida { m, salida: tape::decode(&io.output[1]) }
}

fn correr(fuente: &str) -> Corrida {
    correr_con(fuente, "")
}

/// Valor de una variable especial (a..z) al terminar.
fn especial(c: &Corrida, l: char) -> f64 {
    let k = mercury_autocode::generar::letra_especial(l, false);
    Float::from_long(c.m.store.long(k as usize)).to_f64()
}

fn indice(c: &Corrida, l: char) -> i32 {
    mercury_core::store::signed10(c.m.store.short(116 + (l as usize - 'i' as usize)))
}

fn cerca(a: f64, b: f64) -> bool {
    if b == 0.0 {
        a.abs() < 1e-8
    } else {
        ((a - b) / b).abs() < 2e-8
    }
}

#[test]
fn numero_aleatorio_del_manual() {
    // Cap. 2: "i = 29, j = 955, k = 202, l = 140, 400, 58, 410 (a) sets a to 435".
    // Los índices i, j, k, l forman el largo 58 = 43^5 con exponente 29.
    let c = correr(
        "
        i = 29
        j = 955
        k = 202
        l = 140
        400, 58
        410 (a)
        end
    ",
    );
    assert_eq!(especial(&c, 'a'), 147_008_443.0);
}

#[test]
fn indices_y_aritmetica() {
    let c = correr(
        "
        m = 3
        n = 4
        i = 2mn + m + n + 1
        j = 10i - 7
        k = -j + 5
        x = 2.5
        y = 2mn x + 0.01m + i
        z = x/4 - 3/x
        end
    ",
    );
    assert_eq!(indice(&c, 'i'), 32);
    assert_eq!(indice(&c, 'j'), 313);
    assert_eq!(indice(&c, 'k'), -308);
    assert!(cerca(especial(&c, 'y'), 2.0 * 12.0 * 2.5 + 0.03 + 32.0));
    assert!(cerca(especial(&c, 'z'), 2.5 / 4.0 - 3.0 / 2.5));
}

#[test]
fn polinomio_en_autocode_y_en_maquina() {
    // Apéndice 3: y = a0 + a1 x + ... + a10 x^10, en Autocode y con
    // instrucciones de máquina.
    let datos = "a -> 10\nx = 0.5\nr = 0(1)10\nar = r + 1\nrepeat\n";
    let v1 = format!("{datos}u = a10\nr = 9(-1)0\nu = ux + ar\nrepeat\nend\n");
    let v2 = format!(
        "{datos}300, 10\n407 (a0)\n1) 330, 1\n500 (x)\n427 (a0)\n280 (1)\n410 (u)\nend\n"
    );
    let esperado: f64 = (0..=10).map(|r| (r + 1) as f64 * 0.5f64.powi(r)).sum();
    for v in [v1, v2] {
        let c = correr(&v);
        assert!(cerca(especial(&c, 'u'), esperado), "{} en vez de {esperado}", especial(&c, 'u'));
    }
}

#[test]
fn suma_de_la_superdiagonal() {
    // Cap. 1: suma de los elementos por encima de la diagonal de una matriz n × n.
    let c = correr(
        "
        a -> 24
        n = 5
        i = 0(1)24
        ai = i
        repeat
        x = 0
        i = 2(1)n
        p = i - 1
        j = 1(1)p
        q = np + j - 1
        x = x + aq
        repeat
        repeat
        end
    ",
    );
    let n = 5;
    let mut s = 0;
    for i in 2..=n {
        let p = i - 1;
        for j in 1..=p {
            s += n * p + j - 1;
        }
    }
    // Con = cada suma se redondea forzando un 1: no da el entero exacto.
    assert!(cerca(especial(&c, 'x'), s as f64));
}

#[test]
fn saltos_y_marcas() {
    let c = correr(
        "
        x = -2
        jump 1, x ≥ 0
        f = 1
        jump 2
     1) f = 2
     2) i = 0
     3) i = i + 1
        jump 3, 5 > i
        n) = 4)
        jump 10
     4) g = 7
        jump 5, g = 7
        g = 0
     5) jump 6, g ≠ 7
        h = 1
        end
     6) h = 2
        end
    10) jump (n)
    ",
    );
    assert_eq!(especial(&c, 'f'), 1.0);
    assert_eq!(indice(&c, 'i'), 5);
    assert_eq!(especial(&c, 'g'), 7.0);
    assert_eq!(especial(&c, 'h'), 1.0);
}

#[test]
fn funciones_simples() {
    let c = correr(
        "
        x = -3.75
        a = φ int pt(x)
        b = φ fr pt(x)
        c = φ mod(x)
        d = φ sign(x)
        e = φ divide(1 + x, 2)
        i = φ int pt(10.7)
        f = φ parity(i)
        end
    ",
    );
    assert_eq!(especial(&c, 'a'), -4.0);
    assert!(cerca(especial(&c, 'b'), 0.25));
    assert_eq!(especial(&c, 'c'), 3.75);
    assert_eq!(especial(&c, 'd'), -1.0);
    assert!(cerca(especial(&c, 'e'), -1.375));
    assert_eq!(indice(&c, 'i'), 10);
    assert_eq!(especial(&c, 'f'), 1.0);
}

#[test]
fn fibonacci_impreso() {
    let c = correr(
        "
        s = 1
        t = 1
        i = 2(1)10
        r = s + t
        s = t
        t = r
        print (i) 2, 0
        print (r) 3, 0
        newline
        repeat
        end
    ",
    );
    let mut esperado = String::new();
    let (mut s, mut t) = (1, 1);
    for i in 2..=10 {
        let r = s + t;
        s = t;
        t = r;
        esperado.push_str(&format!("{:>3}  {:>4}  \n", i, r));
    }
    assert_eq!(c.salida, esperado);
}

#[test]
fn leer_datos_e_interrogacion() {
    let c = correr_con(
        "
        read (n)
        x = 0
        i = 1(1)n
        read (y)
        x = x + y
        repeat
        x = x/n ?
        end
    ",
        "4\n1.5\n2.5\n-1\n10\n",
    );
    assert!(cerca(especial(&c, 'x'), 3.25));
    // ? imprime en forma flotante con 10 decimales; la división pierde algo
    // en la novena cifra.
    assert!(c.salida.starts_with("\n 3.2"), "{:?}", c.salida);
    let (mantisa, exp) = c.salida.trim().split_once(',').unwrap();
    assert!(cerca(mantisa.parse::<f64>().unwrap(), 3.25));
    assert_eq!(exp.trim(), "0");
}

#[test]
fn capitulos_across_down_up() {
    let c = correr(
        "
        chapter 1
        x -> 9
     1) x3 = 7
        y = 1
        up
     2) y = y + 10
        across 3/0
        close
        chapter 0
        variables 1
        down 1/1
        z = x3 + y
        across 2/1
     3) end
        close
    ",
    );
    assert!(cerca(especial(&c, 'z'), 8.0));
    assert!(cerca(especial(&c, 'y'), 11.0));
}

#[test]
fn integral_de_sievert() {
    // Ejemplo del cap. 1 (y samples/autocode/int.mer), con a = 1, sin
    // funciones: cos y exp se cargan desde la cinta de datos.
    // Aquí sólo se comprueba la estructura con una tabla dada.
    let c = correr_con(
        "
        f -> 180
        h = π/360
        s = 0(1)179
        read (fs)
        repeat
        f180 = 0
        y = 0
        r = 1(1)90
        s = 2r - 1
        f = f(s-1) + 4fs + f(s+1)
        y = y + hf/3
        repeat
        end
    ",
        &(0..180)
            .map(|s| {
                let x = s as f64 * std::f64::consts::PI / 360.0;
                format!("{:.9}\n", (-1.0 / x.cos()).exp())
            })
            .collect::<String>(),
    );
    // Simpson con paso h sobre [0, π/2] (el último punto vale 0).
    let h = std::f64::consts::PI / 360.0;
    let f = |s: usize| if s >= 180 { 0.0 } else { (-1.0 / (s as f64 * h).cos()).exp() };
    let mut y = 0.0;
    for r in 1..=90 {
        let s = 2 * r - 1;
        y += h * (f(s - 1) + 4.0 * f(s) + f(s + 1)) / 3.0;
    }
    assert!(((especial(&c, 'y') - y) / y).abs() < 1e-7, "{} en vez de {y}", especial(&c, 'y'));
}

#[test]
fn polinomio_con_ciclo_y_maquina() {
    // García Camarero, Apéndice 2: y = a0 x^10 + a1 x^9 + ... + a10 con un
    // ciclo de Autocode alrededor de instrucciones de máquina.
    let c = correr(
        "
        a -> 10
        x = 0.5
        r = 0(1)10
        ar = r + 1
        repeat
        400 (a0)
        i = 1(1)10
        500 (x)
        420 (ai)
        repeat
        410 (y)
        end
    ",
    );
    let esperado: f64 = (0..=10).map(|r| (r + 1) as f64 * 0.5f64.powi(10 - r)).sum();
    assert!(cerca(especial(&c, 'y'), esperado), "{} en vez de {esperado}", especial(&c, 'y'));
}

#[test]
fn contornos_de_whitfield() {
    // Programa de prueba del compilador de Edimburgo (samples/autocode/contour.mer):
    // dos capítulos, instrucciones de máquina con B5 y 625 para elegir el carácter.
    let fuente = include_str!("../../../samples/autocode/contour.mer");
    let img = compilar(fuente).unwrap_or_else(|e| panic!("{e}"));
    let datos: String = img
        .datos
        .as_deref()
        .unwrap()
        .lines()
        .map(|l| format!("{}\n", l.split("//").next().unwrap().trim()))
        .collect();
    let mut m = Machine::new(Default::default());
    img.cargar(&mut m);
    let mut io = BufferIo::with_tape(&tape::encode(&datos).unwrap());
    // Al terminar el dibujo vuelve a leer alturas y se acaba la cinta.
    assert_eq!(m.run(&mut io, 200_000_000), Some(Stop::InputExhausted { channel: 1 }));
    let salida = tape::decode(&io.output[1]);
    let filas: Vec<&str> = salida.lines().filter(|l| l.starts_with('.')).collect();
    assert_eq!(filas.len(), 52, "{salida}");
    assert_eq!(filas[0], ".".repeat(53));
    assert_eq!(filas[1], ".4 5  6   7          7     6       5       4  3  2 .");
    assert_eq!(filas[45], ".1      2               2                   1      .");
}

#[test]
fn caminos_varios() {
    // n) = m), hoot, », ? de un índice, -a/b, divisor con sufijo variable,
    // title y psa.
    let fuente = "
        title
        prueba de caminos
        x -> 4
        i = 0(1)4
        xi = i + 1
        repeat
        m = 2
        n) = m)
        jump (n)
     1) a = 1
        jump 3
     2) a = 2
     3) hoot
        b ~ 3 + 4
        k = 7 ?
        c = -a/b
        i = 3
        d = 4/xi
        psa
        end
    ";
    let img = compilar(fuente).unwrap();
    assert!(img.salida_traduccion.starts_with("prueba de caminos\n0 "), "{}", img.salida_traduccion);
    let c = correr(fuente);
    assert_eq!(especial(&c, 'a'), 2.0);
    assert_eq!(especial(&c, 'b'), 7.0); // sin redondeo, exacto
    assert!(cerca(especial(&c, 'c'), -2.0 / 7.0));
    assert!(cerca(especial(&c, 'd'), 1.0));
    assert_eq!(indice(&c, 'k'), 7);
    assert_eq!(c.salida, "\n 7  ");
    // hoot: unos 1000 impulsos en un segundo.
    let img = compilar(fuente).unwrap();
    let mut m = Machine::new(Default::default());
    img.cargar(&mut m);
    let mut io = BufferIo::default();
    m.run(&mut io, 1_000_000);
    assert_eq!(io.hoots.len(), 1000);
    let dur = io.hoots[999] - io.hoots[0];
    assert!((900_000..1_000_000).contains(&dur), "{dur} µs");
}

#[test]
fn quickies_y_errores() {
    assert!(compilar("x = 1\nφ sqrt\nφ exp\nend\n").is_ok());
    let e = compilar("φ6 (1000) xk, 10\nend\n").unwrap_err();
    assert!(e.msg.contains("φ6"), "{e}");
    let e = compilar("y = φ max(x0, 1, 3)\nend\n").unwrap_err();
    assert!(e.msg.contains("max"), "{e}");
    let e = compilar("i = 1(1)3\nend\n").unwrap_err();
    assert!(e.msg.contains("repeat"), "{e}");
    let e = compilar("x3 = 1\nend\n").unwrap_err();
    assert!(e.msg.contains("sin reservar"), "{e}");
}

#[test]
fn integral_de_sievert_completa() {
    // El ejemplo del cap. 1 tal cual (samples/autocode/int.mer), con a = 1:
    // usa φ cos y φ exp, print y ciclos. Se compara la tabla impresa con el
    // mismo cálculo en el anfitrión.
    let fuente = format!("a = 1\n{}", include_str!("../../../samples/autocode/int.mer"));
    let c = correr(&fuente);
    let h = std::f64::consts::PI / 360.0;
    let f = |s: usize| if s >= 180 { 0.0 } else { (-1.0 / (s as f64 * h).cos()).exp() };
    let mut y = 0.0;
    let filas: Vec<&str> = c.salida.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(filas.len(), 90);
    for (k, fila) in filas.iter().enumerate() {
        let r = k + 1;
        let s = 2 * r - 1;
        y += h * (f(s - 1) + 4.0 * f(s) + f(s + 1)) / 3.0;
        let mut campos = fila.split_whitespace();
        assert_eq!(campos.next().unwrap().parse::<usize>().unwrap(), r);
        let v: f64 = campos.next().unwrap().parse().unwrap();
        assert!((v - y).abs() <= 1.5e-6, "fila {r}: {v} en vez de {y:.6}");
    }
}

#[test]
fn funciones_en_autocode() {
    let c = correr(
        "
        x = 2
        a = φ sqrt(x)
        b = φ exp(1)
        c = φ log(10)
        d = φ sin(π/6)
        e = φ cos(π/3)
        f = φ tan(π/4)
        g = φ arctan(-1, 1)
        h = φ radius(3, 4)
        u = φ divide(1 + 1/x, x)
        φ sqrt
        end
    ",
    );
    let pi = std::f64::consts::PI;
    for (l, v) in [
        ('a', 2f64.sqrt()),
        ('b', 1f64.exp()),
        ('c', 10f64.ln()),
        ('d', 0.5),
        ('e', 0.5),
        ('f', 1.0),
        ('g', 0.75 * pi),
        ('h', 5.0),
        ('u', 0.75),
    ] {
        assert!(cerca(especial(&c, l), v), "{l} = {} en vez de {v}", especial(&c, l));
    }
}
