//! examples/elecciones: consulta de resultados electorales en Autocode, con
//! la cinta generada de los atlas de 257arg. Compara lo que perfora la
//! máquina con las salidas guardadas en examples/elecciones/salidas, que se
//! controlaron contra los porcentajes del atlas (examples/elecciones/controlar.mjs).

use mercury_autocode::compilar;
use mercury_core::io::BufferIo;
use mercury_core::tape;
use mercury_core::{Machine, Stop};

const PROGRAMA: &str = include_str!("../../../examples/elecciones/elecciones.mer");
const CINTA: &str = include_str!("../../../examples/elecciones/cinta.txt");

fn consultar(consulta: &str) -> String {
    let img = compilar(PROGRAMA).unwrap_or_else(|e| panic!("{e}"));
    let mut m = Machine::new(Default::default());
    img.cargar(&mut m);
    let datos = format!("{consulta}\n{CINTA}");
    let mut io = BufferIo::with_tape(&tape::encode(&datos).unwrap());
    assert_eq!(m.run(&mut io, 50_000_000), Some(Stop::Stop99), "consulta {consulta}");
    tape::decode(&io.output[1])
}

/// Sin espacios al final de las líneas ni líneas vacías al final.
fn normalizar(s: &str) -> String {
    s.lines().map(str::trim_end).collect::<Vec<_>>().join("\n").trim_end().to_string()
}

fn comparar(consulta: &str, guardada: &str) {
    let salida = consultar(consulta);
    assert_eq!(normalizar(&salida), normalizar(guardada), "consulta {consulta}:\n{salida}");
}

#[test]
fn catalogo() {
    comparar("0 0 0", include_str!("../../../examples/elecciones/salidas/catalogo.txt"));
}

#[test]
fn total_del_pais_1958() {
    comparar("18 1 0", include_str!("../../../examples/elecciones/salidas/1958_pais.txt"));
}

#[test]
fn distritos_1958() {
    comparar("18 2 0", include_str!("../../../examples/elecciones/salidas/1958_distritos.txt"));
}

#[test]
fn ficha_de_buenos_aires_1962() {
    comparar("19 3 1", include_str!("../../../examples/elecciones/salidas/1962_buenos_aires.txt"));
}

#[test]
fn diferencia_1962() {
    comparar("19 4 0", include_str!("../../../examples/elecciones/salidas/1962_diferencia.txt"));
}

#[test]
fn mapas() {
    comparar("18 5 0", include_str!("../../../examples/elecciones/salidas/1958_mapa.txt"));
    comparar("19 5 0", include_str!("../../../examples/elecciones/salidas/1962_mapa.txt"));
    comparar("20 5 0", include_str!("../../../examples/elecciones/salidas/1965_mapa.txt"));
    let mapa1965 = consultar("20 5 0");
    assert!(mapa1965.contains("30.92  UNION POPULAR (UP)"));
    assert!(mapa1965.contains("29.74  UCR DEL PUEBLO (UCRP)"));
}

#[test]
fn porcentajes_del_atlas_1958() {
    // Cartel nacional del atlas de 1958 (sobre votos positivos) y la
    // participación, calculados por la máquina con votos y denominador.
    let s = consultar("18 1 0");
    for (votos, pct) in [("4070398", "49.49"), ("2617693", "31.83"), ("344425", "4.19"), ("31987", "0.39"), ("7596", "0.09")] {
        assert!(s.lines().any(|l| l.split_whitespace().take(2).eq([votos, pct])), "{votos} {pct}:\n{s}");
    }
    assert!(s.contains("90.86  PARTICIPACION"), "{s}");
}

#[test]
fn consultas_sin_respuesta() {
    assert!(consultar("18 3 30").contains("NO HAY DISTRITO"));
    assert!(consultar("7 1 0").contains("NO ESTA"));
}

#[test]
fn consulta_1967_cinta_5_canales() {
    let img = compilar(PROGRAMA).unwrap();
    let mut m = Machine::new(Default::default());
    img.cargar(&mut m);
    let mut io = BufferIo::with_tape(&tape::encode("1967 1 0\n").unwrap());
    assert_eq!(m.run(&mut io, 1_000_000), Some(Stop::Stop99));
    let esperado: Vec<u16> = [
        31, 12, 6, 31, 0, // N
        14, 17, 17, 14, 0, // O
        0, 0, 0, 0, 0, // espacio
        15, 20, 20, 15, 0, // A
        31, 12, 6, 31, 0, // N
        31, 17, 17, 14, 0, // D
        15, 20, 20, 15, 0, // A
        0, 1, 6, 0, 0, // ,
        0, 0, 0, 0, 0, // espacio
        31, 8, 8, 31, 0, // M
        17, 31, 17, 0, 0, // I
        0, 0, 0, 0, 0, // espacio
        14, 17, 21, 23, 0, // G
        31, 21, 21, 17, 0, // E
        31, 12, 6, 31, 0, // N
        31, 21, 21, 17, 0, // E
        31, 20, 22, 9, 0, // R
        15, 20, 20, 15, 0, // A
        31, 1, 1, 1, 0, // L
    ].to_vec();
    assert_eq!(io.output[1], esperado);
}
