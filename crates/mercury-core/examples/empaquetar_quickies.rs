//! Reconstruye los sectores de las Quickies a partir de las fuentes y R67.
//! cargo run -p mercury-core --example empaquetar_quickies

use mercury_core::asm::{assemble_at, assemble_sector};
use std::{fmt::Write, fs, path::Path};

const ORIGEN: usize = 128;
const PRIMERO: usize = 47;
const SECTORES: usize = 17;
const CORTOS: usize = 128;

fn fuente_relativa(fuente: &str) -> String {
    let mut relativa = String::new();
    for linea in fuente.lines() {
        if linea.trim() == "@128" {
            continue;
        }
        let (codigo, comentario) = linea.split_once(';').unwrap_or((linea, ""));
        let marca = comentario.split('|').next().unwrap().trim();
        if matches!(marca, "reubicar" | "reubicar largo") {
            let (funcion, resto) = codigo.trim().split_once(' ').unwrap();
            let operando = resto.split_whitespace().next().unwrap();
            let sufijo = &resto.trim()[operando.len()..];
            let corto = operando.ends_with('+');
            let base = operando.trim_end_matches('+').parse::<i32>().unwrap();
            let delta = if marca == "reubicar largo" {
                ORIGEN / 2
            } else {
                ORIGEN
            };
            let direccion = 2 * (base - delta as i32) + i32::from(corto);
            let signo = if direccion < 0 { "-" } else { "" };
            let mas = if direccion % 2 != 0 { "+" } else { "" };
            writeln!(
                relativa,
                "{funcion} {signo}{}{mas}{sufijo}",
                direccion.abs() / 2
            )
            .unwrap();
        } else {
            writeln!(relativa, "{linea}").unwrap();
        }
    }
    relativa
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raiz = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../input_routine");
    let salida = raiz.join("enlazados");
    let tabla = assemble_sector(&fs::read_to_string(salida.join("sector42.txt"))?)?;
    // R67, entrada v12, PDF pág. 91. La primera fila es Q0
    // (impresión n), que ya está en el sector 29.
    let tabla = &tabla[2 * 35..][..40];
    let mut imagen = [None; SECTORES * CORTOS];
    for (numero, fila) in tabla.chunks_exact(2).enumerate().skip(1) {
        if fila[0] == 0 {
            continue;
        }
        let inicio_sector = (usize::from(fila[0]) - PRIMERO) * CORTOS;
        let limite = inicio_sector + usize::from(fila[1] >> 8) * CORTOS;
        let indice = usize::from(fila[1] & 255);
        let inicio = inicio_sector + indice;
        let fuente = fs::read_to_string(raiz.join(format!("quickies/{numero:02}.mer")))?;
        let absoluta = assemble_at(&fuente, ORIGEN as u16)?;
        let relativa = assemble_at(&fuente_relativa(&fuente), 0)?;
        assert_eq!(absoluta.shorts.len(), relativa.shorts.len(), "Q{numero}");
        let salida_de_prueba = usize::from(fuente.trim_end().ends_with("; salida de prueba")) * 2;
        let longitud = relativa.shorts.last().unwrap().0 + 1 - salida_de_prueba;
        let longitud = longitud.div_ceil(2) * 2;
        assert!(inicio + 2 + longitud <= limite, "Q{numero} excede su tramo");
        let mut indicadores = vec![0; longitud.div_ceil(10)];
        let mut codigo = vec![0; longitud];
        for (&(i, base), &(j, reubicado)) in relativa.shorts.iter().zip(&absoluta.shorts) {
            assert_eq!(j, i + 2 * ORIGEN, "Q{numero}");
            if i >= longitud {
                continue;
            }
            codigo[i] = base;
            let delta = reubicado.wrapping_sub(base) & 1023;
            let indicador = match usize::from(delta) {
                0 => 0,
                ORIGEN => 1,
                n if n == ORIGEN / 2 => 2,
                n if n == 2 * ORIGEN => 3,
                _ => panic!("Q{numero}, corto {i}: desplazamiento inválido {delta}"),
            };
            assert!(
                i % 2 != 0 || indicador == 0,
                "Q{numero}, corto izquierdo {i}"
            );
            indicadores[i / 10] |= indicador << (2 * ((i / 2) % 5));
        }
        let mut plantar = |i: usize, palabra| {
            assert!(
                imagen[i].replace(palabra).is_none(),
                "Q{numero}: superposición en {}.{}",
                PRIMERO + i / CORTOS,
                i % CORTOS
            );
        };
        for (i, indicador) in indicadores.into_iter().enumerate() {
            plantar(inicio - i, indicador);
        }
        plantar(inicio + 1, (indice + longitud) as u16);
        for (i, palabra) in codigo.into_iter().enumerate() {
            plantar(inicio + 2 + i, palabra);
        }
    }
    for (sector, cortos) in imagen.chunks_exact(CORTOS).enumerate() {
        let n = PRIMERO + sector;
        let mut texto =
            format!("; Sector {n}. Reconstruido con empaquetar_quickies y la tabla de R67.\n");
        for (i, palabra) in cortos.iter().enumerate() {
            if let Some(valor) = palabra {
                let mas = if i % 2 == 0 { "" } else { "+" };
                writeln!(texto, "{}{mas} ={valor}", i / 2).unwrap();
            }
        }
        if cortos.iter().all(Option::is_none) {
            texto.push_str("; La tabla de R67 de septiembre de 1960 no usa este sector.\n");
        }
        fs::write(salida.join(format!("sector{n:02}.txt")), texto)?;
    }
    Ok(())
}
