//! Pruebas de la reconstrucción del Nim (Marienbad) de Wilfred Durán (`examples/nim.mer`)
//! y del ejemplo de cinta sin datos personales (`examples/nombre_ejemplo.mer`).

use mercury_core::io::BufferIo;
use mercury_core::{asm, tape, Config, Machine, Stop};

const FUENTE: [(&str, [u16; 4]); 24] = [
    (" ", [0, 0, 0, 0]),
    ("A", [15, 20, 20, 15]),
    ("C", [14, 17, 17, 17]),
    ("D", [31, 17, 17, 14]),
    ("E", [31, 21, 21, 17]),
    ("F", [31, 20, 20, 16]),
    ("G", [14, 17, 21, 23]),
    ("H", [31, 4, 4, 31]),
    ("I", [17, 31, 17, 0]),
    ("J", [2, 1, 17, 30]),
    ("L", [31, 1, 1, 1]),
    ("M", [31, 8, 8, 31]),
    ("N", [31, 12, 6, 31]),
    ("O", [14, 17, 17, 14]),
    ("P", [31, 20, 20, 8]),
    ("Q", [14, 17, 19, 13]),
    ("R", [31, 20, 22, 9]),
    ("S", [9, 21, 21, 18]),
    ("T", [16, 31, 16, 16]),
    ("U", [30, 1, 1, 30]),
    ("V", [28, 3, 3, 28]),
    ("Y", [24, 7, 4, 24]),
    ("Z", [19, 21, 25, 17]),
    ("!", [0, 29, 0, 0]),
];

fn decodificar_longitudinal(filas: &[u16]) -> String {
    assert_eq!(filas.len() % 5, 0, "largo {} no es múltiplo de 5 columnas", filas.len());
    let mut texto = String::new();
    for celda in filas.chunks_exact(5) {
        assert_eq!(celda[4], 0, "columna separadora no nula en celda {:?}", celda);
        let patron = [celda[0], celda[1], celda[2], celda[3]];
        let &(ch, _) = FUENTE
            .iter()
            .find(|(_, p)| *p == patron)
            .unwrap_or_else(|| panic!("glifo desconocido {:?}", patron));
        texto.push_str(ch);
    }
    texto
}

fn iniciar_nim() -> (Machine, BufferIo) {
    let prog = asm::assemble(include_str!("../../../examples/nim.mer")).unwrap();
    let mut m = Machine::new(Config::default());
    prog.load(&mut m.store);
    m.goto(prog.start);
    let mut io = BufferIo::default();
    assert_eq!(m.run(&mut io, 20_000), Some(Stop::Stop99));
    (m, io)
}

fn responder_llaves(m: &mut Machine, io: &mut BufferIo, valor: u16) -> Stop {
    io.switches = valor;
    let first = m.prepulse(io);
    match first {
        Err(s) => s,
        Ok(()) => m.run(io, 200_000).expect("se esperaba parada 99"),
    }
}

fn cargar_tablero(m: &mut Machine, io: &mut BufferIo, filas: [u16; 4]) -> Stop {
    assert_eq!(m.b[5], 1);
    for (idx, &f) in filas.iter().enumerate() {
        assert_eq!(m.b[5], (idx + 1) as u16);
        let s = responder_llaves(m, io, f);
        assert_eq!(s, Stop::Stop99);
    }
    Stop::Stop99
}

#[test]
fn glifos_longitudinales_sin_colisiones() {
    for (i, (a, pa)) in FUENTE.iter().enumerate() {
        for (b, pb) in FUENTE.iter().skip(i + 1) {
            assert_ne!(pa, pb, "colisión entre {a:?} y {b:?}");
        }
    }
}

#[test]
fn posicion_segura_juega_optimo_y_gana() {
    let (mut m, mut io) = iniciar_nim();
    cargar_tablero(&mut m, &mut io, [1, 3, 5, 7]);

    // 1 ^ 3 ^ 5 ^ 7 = 0 => Clementina tiene posición ganadora al mover segunda.
    assert_eq!(m.b[5], 5);
    assert_eq!(decodificar_longitudinal(&io.output[1]).trim(), "JUEGUE QUE YO LE GANARE");
    assert_eq!([m.b[1], m.b[2], m.b[3], m.b[4]], [1, 21, 341, 127]);
    assert_eq!(io.displays.last().copied(), Some([127, 341, 21, 1]));

    // Simular un partido completo jugando siempre la primera fila no vacía.
    while m.b[5] == 5 {
        let mut fila_elegida = None;
        for f in 0..4usize {
            let v = m.store.short(20 + f);
            if v > 0 {
                fila_elegida = Some((f + 1) as u16);
                break;
            }
        }
        let fila = fila_elegida.expect("debería quedar algún palillo");
        assert_eq!(responder_llaves(&mut m, &mut io, fila), Stop::Stop99);
        assert_eq!(m.b[5], 6);
        // Sacar 1 palillo en cada turno hasta que Clementina vacíe todo.
        let s = responder_llaves(&mut m, &mut io, 1);
        assert_eq!(s, Stop::Stop99);
        if m.b[5] == 5 {
            // Cada vez que devuelve el turno en posición segura, el nim-sum es 0.
            let xor = m.store.short(20)
                ^ m.store.short(21)
                ^ m.store.short(22)
                ^ m.store.short(23);
            assert_eq!(xor, 0);
        }
    }

    assert_eq!(m.b[5], 7);
    let texto = decodificar_longitudinal(&io.output[1]);
    assert!(texto.contains("QUITE "), "{texto}");
    assert!(texto.contains(" DE LA FILA "), "{texto}");
    assert!(texto.ends_with("GANE ! QUIERO OTRO "), "{texto:?}");
    assert_eq!([m.b[1], m.b[2], m.b[3], m.b[4]], [0, 0, 0, 0]);
}

#[test]
fn posicion_insegura_saca_un_solo_palillo() {
    let (mut m, mut io) = iniciar_nim();
    cargar_tablero(&mut m, &mut io, [1, 2, 4, 0]);

    assert_eq!(m.b[5], 5);
    assert_eq!(decodificar_longitudinal(&io.output[1]).trim(), "MI POSICION ES MALA PERO CUIDESE");

    // Humano quita 1 de fila 3 -> deja [1, 2, 3, 0], nim-sum == 0 (mala para Clementina).
    assert_eq!(responder_llaves(&mut m, &mut io, 3), Stop::Stop99);
    assert_eq!(responder_llaves(&mut m, &mut io, 1), Stop::Stop99);

    assert_eq!(m.b[5], 5);
    let texto = decodificar_longitudinal(&io.output[1]);
    // Fila con más palillos era la 3 (3 palillos): saca UNO de la fila TRES.
    assert!(texto.contains("QUITE UNO DE LA FILA TRES"), "{texto}");
    assert_eq!(
        [m.store.short(20), m.store.short(21), m.store.short(22), m.store.short(23)],
        [1, 2, 2, 0]
    );
}

#[test]
fn detecta_trampas_y_valores_fuera_de_rango() {
    // En la carga inicial, un valor > 10 vuelve a pedir la misma fila.
    let (mut m, mut io) = iniciar_nim();
    assert_eq!(responder_llaves(&mut m, &mut io, 15), Stop::Stop99);
    assert_eq!(m.b[5], 1);
    assert_eq!(responder_llaves(&mut m, &mut io, 600), Stop::Stop99);
    assert_eq!(m.b[5], 1);

    // Probar jugadas ilegales con signo/rango distintos.
    for (fila, cant) in [(0, 1), (5, 1), (512, 1), (1, 0), (1, 4), (1, 700)] {
        let (mut m, mut io) = iniciar_nim();
        cargar_tablero(&mut m, &mut io, [2, 2, 0, 0]);
        assert_eq!(responder_llaves(&mut m, &mut io, fila), Stop::Stop99);
        assert_eq!(responder_llaves(&mut m, &mut io, cant), Stop::Stop99);
        assert_eq!(m.b[5], 9, "fila={fila}, cant={cant}");
        let texto = decodificar_longitudinal(&io.output[1]);
        assert!(texto.contains("NO JUEGO MAS PORQUE HIZO TRAMPA"), "{texto}");
    }
}

#[test]
fn victoria_humana_y_reinicio_con_prepulse() {
    let (mut m, mut io) = iniciar_nim();
    cargar_tablero(&mut m, &mut io, [0, 0, 0, 3]);
    assert_eq!(responder_llaves(&mut m, &mut io, 4), Stop::Stop99);
    assert_eq!(responder_llaves(&mut m, &mut io, 3), Stop::Stop99);
    assert_eq!(m.b[5], 8);
    assert!(decodificar_longitudinal(&io.output[1]).contains("USTED GANO ! QUIERO REVANCHA"));

    // Otro PREPULSE vuelve a pedir la fila 1 de un partido nuevo.
    assert_eq!(responder_llaves(&mut m, &mut io, 0), Stop::Stop99);
    assert_eq!(m.b[5], 1);
}

#[test]
fn nombre_ejemplo_imprime_clementina() {
    let prog = asm::assemble(include_str!("../../../examples/nombre_ejemplo.mer")).unwrap();
    let mut m = Machine::new(Config::default());
    prog.load(&mut m.store);
    m.goto(prog.start);
    let mut io = BufferIo::default();
    assert_eq!(m.run(&mut io, 10_000), Some(Stop::Stop99));
    assert_eq!(tape::decode(&io.output[1]), "CLEMENTINA");
}
