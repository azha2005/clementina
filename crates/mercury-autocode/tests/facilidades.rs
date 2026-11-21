//! Facilidades de los manuales compiladas y ejecutadas en la Mercury.

use mercury_autocode::{compilar, generar::letra_especial};
use mercury_core::{acc::Float, io::BufferIo, store::signed10, Machine, Stop};

fn ejecutar(fuente: &str) -> Machine {
    let imagen = compilar(fuente).unwrap();
    let mut m = Machine::new(Default::default());
    imagen.cargar(&mut m);
    assert_eq!(m.run(&mut BufferIo::default(), 100_000), Some(Stop::Stop99), "{}", imagen.listado);
    m
}

fn especial(m: &Machine, letra: char) -> f64 {
    Float::from_long(m.store.long(letra_especial(letra, false).into())).to_f64()
}

#[test]
fn polinomio_extremos_y_check() {
    let m = ejecutar("
        x -> 4
        x0 » 7
        x1 » -4
        x2 » 7
        x3 » -4
        x4 » 9
        i = 3
        j = φ max(x0,0,i)
        k = φ min(x0,0,i)
        l = φ max(x0, i, 4)
        a » φ poly(2) x0, i
        b » φ poly(100) x0, 0
        check(a, -5, 0.01, 1)
        end
        1) check(a, -4, 1, 3)
        c » 32
        3) end
    ");
    assert_eq!((1..=3).map(|k| signed10(m.store.short(116 + k))).collect::<Vec<_>>(), [0, 1, 4]);
    assert_eq!(especial(&m, 'a'), -5.0);
    assert_eq!(especial(&m, 'b'), 7.0);
    assert_eq!(especial(&m, 'c'), 32.0);
}

#[test]
fn preservar_en_dos_niveles() {
    let m = ejecutar("
        chapter 2
        1) a » 300
        i = 300
        t = 300
        up
        close
        chapter 1
        1) a » 200
        i = 200
        t = -200
        preserve
        down 1/2
        restore
        check(a, 200, .01, 2)
        end
        2) jump 3, t = -200
        end
        3) up
        close
        chapter 0
        a » 100
        i = 100
        t = -100
        preserve
        down 1/1
        restore
        end
        close
    ");
    assert_eq!(especial(&m, 'a'), 100.0);
    assert_eq!(signed10(m.store.short(116)), 100);
    assert_eq!(signed10(m.store.short(127)), -100);
}

#[test]
fn transferencias_auxiliares_cruzan_un_sector() {
    let m = ejecutar("
        x -> 36
        y -> 36
        i = 1(1)35
        xi » i
        repeat
        i = 1
        j = 35
        φ7 (31.5) xi, j
        φ6 (31) y1, 35
        φ7 (-1) x3, 2
        φ6 (-1) y0, 1
        φ6 (0) a, 1
        b » y35
        c » y0
        d » y1
        e » y2
        end
    ");
    assert_eq!(especial(&m, 'a'), 4.0);
    assert_eq!(especial(&m, 'b'), 35.0);
    assert_eq!(especial(&m, 'c'), 3.0);
    assert_eq!(especial(&m, 'd'), 1.0);
    assert_eq!(especial(&m, 'e'), 2.0);
}

#[test]
fn cinta_de_direccion_y_programas_de_biblioteca() {
    let m = ejecutar("
        chapter 1
        v -> 1
        1) v1 » v1 + c
        rmp
        close

        programme -2
        chapter 1
        variables 0
        2) v1 » 10
        up
        close

        chapter 0
        v -> 1
        down 2/1 - 2
        rmp
        close

        chapter 0
        variables 1
        n = 1
        c = 2.8
        across 1/1
        close

        chapter 0
        variables 0
        n = 2
        c = 2.9
        across 1/1
        close

        chapter 0
        variables 1
        n = 2
        c = 3.0
        across 1/1
        close
    ");
    let v1 = Float::from_long(m.store.long(513)).to_f64();
    assert!((v1 - 18.7).abs() < 1e-6, "{v1}");
    assert_eq!(m.store.short(203), 3);
}

#[test]
fn runge_kutta_del_capitulo_dos() {
    let m = ejecutar("
        chapter 0
        f -> 2
        g -> 2
        h -> 2
        y -> 2
        n = 2
        h = 0.02
        m = 1(1)2
        i) = m)
        jump (i)
        11) int step (10)
        jump 11, 0.19 > x
        repeat
        end
        10) f1 = y1y1 -1.23 y1y2 + 2.47 y2y2
        f2 = 1.01y1y1 -0.84 y1y2 + 1.59 y2y2
        592,0
        1) x = 0
        y1 = 0
        y2 = 1
        jump 11
        2) x = 0
        y1 = 1
        y2 = 0
        jump 11
        close
    ");
    // Se evalúa en x=0,2. Las condiciones del manual divergen antes de x=1.
    assert!((especial(&m, 'x') - 0.2).abs() < 1e-6);
    let par = [522, 523].map(|i| Float::from_long(m.store.long(i)).to_f64());
    assert!((par[0] - 1.222975816990966).abs() < 1e-6, "{par:?}");
    assert!((par[1] - 0.23237137052983736).abs() < 1e-6, "{par:?}");
}

#[test]
fn un_paso_con_subsecuencia_que_llama_a_division() {
    let m = ejecutar("
        f -> 1
        g -> 1
        h -> 1
        y -> 1
        n = 1
        h » .125
        x » 0
        y1 » 1
        int step (1)
        a » y1
        end
        1) f1 » φ divide(y1,2)
        592,0
    ");
    let esperado = 1.0 + (1..=4).fold((0.0, 1.0), |(s, t), n| {
        let siguiente = t / (16.0 * f64::from(n));
        (s + siguiente, siguiente)
    }).0;
    assert!((especial(&m, 'a') - esperado).abs() < 1e-7, "{} vs {esperado}; {:?}", especial(&m,'a'), [513, 515, 517, 519].map(|i| Float::from_long(m.store.long(i)).to_f64()));
    assert_eq!(especial(&m, 'x'), 0.125);
}

#[test]
fn operaciones_complejas_y_destinos_solapados() {
    let m = ejecutar("
        (a,b) = (3,4)
        (b,a) = (a,b)
        (c,d) = (b,a) + (1,2)
        (c,d) = (c,d) - (1,2)
        (e,f) = (c,d) * (2,-1)
        (e,f) = (e,f) / (2,-1)
        (g,h) = (4,-3) / (-1,-2)
        (u,v) = φ sqrt(-3,-4)
        (w,x) = φ exp(0,1)
        (y,z) = φ log(-3,4)
        end
    ");
    for (letra, esperado) in [
        ('a', 4.0), ('b', 3.0), ('c', 3.0), ('d', 4.0), ('e', 3.0), ('f', 4.0),
        ('g', 0.4), ('h', 2.2), ('u', 1.0), ('v', -2.0), ('w', 1.0f64.cos()),
        ('x', 1.0f64.sin()), ('y', 5.0f64.ln()), ('z', 4.0f64.atan2(-3.0)),
    ] {
        let valor = especial(&m, letra);
        assert!((valor - esperado).abs() < 1e-7, "{letra}: {valor} ≠ {esperado}");
    }
}

#[test]
fn doble_longitud_y_recuperacion_de_la_parte_baja() {
    let m = ejecutar("
        double length
        ((a,b)) = 1 / ((3,0))
        ((c,d)) = ((a,b)) * ((3,0))
        ((e,f)) = ((c,d)) - ((1,0))
        ((g,h)) = ((1,0)) + ((0,0.0000000000000001))
        ((u,v)) = ((g,h)) - ((1,0))
        ((x,y)) = ((v,u))
        end
    ");
    let tercio = especial(&m, 'a') + especial(&m, 'b');
    assert!((tercio - 1.0 / 3.0).abs() < 1e-16);
    assert!((especial(&m, 'e') + especial(&m, 'f')).abs() < 2e-16);
    assert!((especial(&m, 'y') - 1e-16).abs() < 1e-24);
}

#[test]
fn matrices_lectura_combinacion_y_diagonal() {
    let img = compilar("
        a' » 31
        b' » 63
        c' » 95
        d' » 127
        φ10(a',4)
        φ10(b',2)
        c' = φ23(a', 2, b', 2)
        d' = φ14(c', 1, a', 4)
        φ8(c',2,2,2,0)
        end
    ").unwrap();
    let mut m = Machine::new(Default::default());
    img.cargar(&mut m);
    let mut io = BufferIo::with_tape(&mercury_core::tape::encode("1  2  3  4  5  6  ").unwrap());
    assert_eq!(m.run(&mut io, 200_000), Some(Stop::Stop99), "{}", img.listado);
    let leer = |a: usize| {
        let s = m.drum.sector(128 + a / 32);
        let pos = (a % 32) * 4;
        Float::from_long(s[pos..pos + 4].try_into().unwrap()).to_f64()
    };
    assert_eq!([95, 96, 97, 98].map(leer), [11.0, 2.0, 3.0, 16.0]);
    assert_eq!([127, 128, 129, 130].map(leer), [10.0, 0.0, 0.0, 12.0]);
    assert_eq!(mercury_core::tape::decode(&io.output[1]).split_whitespace().collect::<Vec<_>>(), ["11", "2", "3", "16"]);
}

#[test]
fn minimos_cuadrados_del_capitulo_cuatro() {
    let img = compilar("
        programme -501
        chapter 1
        1) u » m
        v » n
        y' » w' + mn
        w' = φ16(a', u, v)
        y' = φ26(w', a', v, v, u)
        x' = φ26(w', b', v, 1, u)
        x' = φ28(y', n, 1)
        up
        close
        chapter 0
        z -> 2
        m = 3
        n = 2
        a' » 0
        b' » 32
        x' » 64
        w' » 96
        φ10(a',6)
        φ10(b',3)
        preserve
        down 1/1 - 501
        restore
        φ6(64) z1,2
        a » z1
        b » z2
        end
        close
    ").unwrap();
    let mut m = Machine::new(Default::default());
    img.cargar(&mut m);
    let datos = mercury_core::tape::encode("1  0  1  1  1  2  1  3  5  ").unwrap();
    assert_eq!(m.run(&mut BufferIo::with_tape(&datos), 100_000), Some(Stop::Stop99), "C={}", m.c);
    assert!((especial(&m, 'a') - 1.0).abs() < 1e-7, "{}", especial(&m, 'a'));
    assert!((especial(&m, 'b') - 2.0).abs() < 1e-7, "{}", especial(&m, 'b'));
}

#[test]
fn producto_con_transpuesta_y_determinante_con_pivoteo() {
    let img = compilar("
        a' » 31
        b' » 64
        φ10(a',4)
        a' = φ16(a',2,2)
        b' = φ27(a',a',2,2,2)
        a = φ25(a',2)
        φ6(b') b,1
        φ6(b'+1) c,1
        φ6(b'+3) d,1
        end
    ").unwrap();
    let mut m = Machine::new(Default::default());
    img.cargar(&mut m);
    let datos = mercury_core::tape::encode("0  2  3  4  ").unwrap();
    assert_eq!(m.run(&mut BufferIo::with_tape(&datos), 100_000), Some(Stop::Stop99), "C={}", m.c);
    assert_eq!(especial(&m, 'a'), -6.0);
    assert_eq!([especial(&m, 'b'), especial(&m, 'c'), especial(&m, 'd')], [9.0, 12.0, 20.0]);
}

#[test]
fn diagonales_pueden_leerse_de_la_propia_salida() {
    let img = compilar("
        a' » 31
        φ10(a',4)
        a' = φ21(a',a',2)
        a' = φ22(a',a',2)
        φ9(a',2,2,2)
        end
    ").unwrap();
    let mut m = Machine::new(Default::default());
    img.cargar(&mut m);
    let datos = mercury_core::tape::encode("1  2  3  4  ").unwrap();
    let mut io = BufferIo::with_tape(&datos);
    assert_eq!(m.run(&mut io, 100_000), Some(Stop::Stop99));
    let resultado = mercury_core::tape::decode(&io.output[1]);
    assert_eq!(resultado.split_whitespace().collect::<Vec<_>>(),
        ["0.00,-128", "2.00,", "0", "3.00,", "0", "4.00,", "0"]);
}

#[test]
fn la_imagen_incluye_solo_las_rutinas_usadas() {
    let sectores = |fuente| {
        let mut sectores = compilar(fuente).unwrap().tambor.into_iter()
            .map(|(sector, _)| sector).filter(|&s| s < 32 || s >= 480).collect::<Vec<_>>();
        sectores.sort_unstable();
        sectores
    };
    assert_eq!(sectores("a » 1\nend"), []);
    assert_eq!(sectores("φ6(0) a,1\nend"), [3]);
    assert_eq!(sectores("(a,b) = (1,2) / (3,4)\nend"), [2, 11, 12]);
    assert_eq!(sectores("φ10(a',2)\nend"), [2, 480, 481, 491, 492, 497, 498, 499]);
}
