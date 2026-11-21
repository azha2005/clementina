//! Comprueba las proyecciones de TINY contra productos escalares y mide sus ciclos.
use mercury_core::{
    acc::{self, AddMode, Float, MulMode},
    asm,
    io::NullIo,
    Machine, Stop,
};
use std::{env, fs, path::Path};

fn nivel_ternario(corto: u16, fila: usize, columna: usize) -> f64 {
    let indice = if fila % 2 == 0 {
        corto >> 5
    } else {
        corto & 31
    };
    f64::from(indice / [1, 3, 9][columna] % 3) - 1.0
}

fn nivel_seq(cortos: &[u16], fila: usize, columna: usize) -> f64 {
    let [a, b, c] = cortos else { unreachable!() };
    let (alto, medio, bajo) = if fila % 2 == 0 {
        (a >> 5, a & 31, b >> 5)
    } else {
        (b & 31, c >> 5, c & 31)
    };
    let nivel = 4 * (alto >> columna & 1) + 2 * (medio >> columna & 1) + (bajo >> columna & 1);
    2.0 * f64::from(nivel) - 7.0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let imagen = args.next().ok_or("falta la imagen de datos del modelo")?;
    let bancos = args
        .next()
        .ok_or("falta el directorio de bancos generados")?;
    let bytes = fs::read(imagen)?;
    if bytes.len() != 512 * 256 {
        return Err("la imagen debe ocupar 512 sectores".into());
    }
    let datos: Vec<_> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    let firma = datos[..6]
        .iter()
        .copied()
        .map(u8::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let (d, hid, grupo, ancho_par, bloques): (usize, usize, usize, usize, usize) =
        match firma.as_slice() {
            b"TINYT1" => (96, 384, 3, 1, 2),
            b"TINYC1" => (80, 256, 5, 3, 2),
            b"TINYD1" => (80, 224, 5, 3, 3),
            _ => return Err("imagen TINY desconocida".into()),
        };
    let ternario = grupo == 3;
    let compacto = bloques == 3;
    let mut m = Machine::new(Default::default());
    for (sector, datos) in datos.chunks_exact(128).enumerate() {
        m.drum.sector_mut(sector).copy_from_slice(datos);
    }
    let bancos = Path::new(&bancos);
    let matriz = asm::assemble(&fs::read_to_string(bancos.join("matriz.mer"))?)?;
    let f1 = if compacto {
        matriz.clone()
    } else {
        asm::assemble(&fs::read_to_string(bancos.join("f1.mer"))?)?
    };
    asm::assemble(&fs::read_to_string(bancos.join("constantes.mer"))?)?.load(&mut m.store);
    asm::assemble("@14.0\n990 0")?.load(&mut m.store);
    m.store.set_short(100, 14 * 64); // retorno de los bancos al Stop99
    m.store.set_long(9, Float::from_f64(0.0).to_long()); // 18

    let mut verificados = 0;
    let mut instrucciones = 0;
    let mut tiempo_us = 0;
    let formas = [(d, d), (d, d / 4), (d, d / 4), (d, d), (d, hid), (hid, d)];
    let nombres = ["q", "k", "v", "o", "f1", "f2"];
    for bloque in 0..bloques {
        for (tipo, (columnas, filas)) in formas.into_iter().enumerate() {
            let i = 6 * bloque + tipo;
            let (sec_p, sec_c, off_p, off_c) = if compacto {
                let campo = 8 + 2 * (27 + 2 * i);
                (
                    datos[campo],
                    datos[campo + 2],
                    datos[campo + 1],
                    datos[campo + 3],
                )
            } else {
                (datos[25 + 2 * i], datos[26 + 2 * i], 0, 0)
            };
            let pesos = usize::from(sec_p) * 128 + usize::from(off_p);
            if pesos == 0 {
                continue;
            }
            let coeficientes = usize::from(sec_c) * 128 + usize::from(off_c);
            let (entrada, salida, banco) = if tipo == 4 {
                (7 * 32, 19 * 32, &f1)
            } else {
                (19 * 32, 7 * 32, &matriz)
            };
            banco.load(&mut m.store);
            let ancho = columnas.div_ceil(grupo) * grupo;
            for caso in 0..3 {
                for (n, valor) in [ancho as u16, filas as u16, sec_p, sec_c, off_p, off_c]
                    .into_iter()
                    .enumerate()
                {
                    m.store.set_short(120 + n, valor);
                }
                let vector: Vec<_> = (0..ancho)
                    .map(|i| {
                        if i < columnas {
                            ((i * 31 + caso * 7) % 97) as f64 / 32.0 - 1.5
                        } else {
                            0.0
                        }
                    })
                    .collect();
                for (i, &v) in vector.iter().enumerate() {
                    m.store.set_long(entrada + i, Float::from_f64(v).to_long());
                }
                m.goto(if compacto && tipo == 4 { 177 } else { 176 });
                let (antes, inicio) = (m.executed, m.time_us);
                assert_eq!(m.run(&mut NullIo, 2_000_000), Some(Stop::Stop99));
                assert_eq!(m.c, 14 * 64 + 1);
                if caso == 0 {
                    let (n, t) = (m.executed - antes, m.time_us - inicio);
                    println!("{bloque}.{} {n:>7} instrucciones {t:>9} µs", nombres[tipo]);
                    instrucciones += n;
                    tiempo_us += t;
                }
                for fila in 0..filas {
                    let mut suma = 0.0;
                    for (grupo, valores) in vector.chunks_exact(grupo).enumerate() {
                        let p = pesos + ancho_par * (grupo * (filas / 2) + fila / 2);
                        for (col, &valor) in valores.iter().enumerate() {
                            let nivel = if ternario {
                                nivel_ternario(datos[p], fila, col)
                            } else {
                                nivel_seq(&datos[p..p + 3], fila, col)
                            };
                            suma += nivel * valor;
                        }
                    }
                    let (escala, sesgo) = if compacto {
                        let p = coeficientes + 2 + 2 * fila;
                        let convertir = |i| {
                            let corto =
                                Float::from_long([datos[coeficientes + i], 0, 0, datos[p + i]]);
                            Float::from_f64(corto.to_f64())
                        };
                        (convertir(0), convertir(1))
                    } else {
                        let p = coeficientes + 8 * fila;
                        (
                            Float::from_long(datos[p..p + 4].try_into()?),
                            Float::from_long(datos[p + 4..p + 8].try_into()?),
                        )
                    };
                    let producto =
                        acc::mul(Float::from_f64(suma), escala, false, MulMode::Unrounded)
                            .map_err(|_| "desborde al multiplicar por la escala")?;
                    let esperado = acc::add(producto, sesgo, false, AddMode::Unrounded)
                        .map_err(|_| "desborde al sumar el sesgo")?
                        .0;
                    assert_eq!(
                        m.store.long(salida + fila),
                        esperado.to_long(),
                        "bloque {bloque}, matriz {}, caso {caso}, fila {fila}",
                        nombres[tipo]
                    );
                    verificados += 1;
                }
            }
        }
    }
    println!("{verificados} resultados exactos; {instrucciones} instrucciones y {tiempo_us} µs por pasada por las matrices únicas");
    Ok(())
}
