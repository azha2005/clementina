//! Compilador de **Mercury Autocode** (R. A. Brooker, Manchester, 1958–1961)
//! para el emulador de Clementina.
//!
//! El traductor original corría en la propia Mercury y no se conserva. Este
//! compilador corre en el anfitrión y produce la imagen que el traductor
//! habría dejado en la máquina: el código de cada capítulo en su lugar del
//! tambor, las constantes y las rutinas de apoyo, todo en código Mercury que
//! ejecuta el emulador.
//!
//! Fuentes: *Mercury Autocode Manual* (CS 242 A, 2.ª ed., 1961, transcripción
//! de Chilton Computing) y E. García Camarero, *AUTOCODE* (Instituto de
//! Cálculo, UBA, 1961), cuyo Apéndice 1 da el mapa de memoria que se sigue.

pub mod generar;
pub mod runtime;
pub mod sintaxis;

use generar::{armar_capitulo, compilar_capitulo, primer_sector, Entradas, ErrorCompilacion};
use mercury_core::Machine;
use sintaxis::{analizar, Linea, Sentencia};
use std::collections::{BTreeSet, HashMap};

/// Resultado de la compilación: lo que hay que poner en la máquina.
#[derive(Debug, Clone, Default)]
pub struct Imagen {
    /// Registros cortos del almacén (páginas 0, 1, 2–14 con el capítulo 0, y 31).
    pub cortos: Vec<(usize, u16)>,
    /// Sectores del tambor con los capítulos traducidos.
    pub tambor: Vec<(usize, Vec<u16>)>,
    /// Registro medio de arranque (primera instrucción del capítulo 0).
    pub arranque: u16,
    /// Lo que el traductor imprimía al leer el programa (`title`, `psa`).
    pub salida_traduccion: String,
    /// Listado del código generado.
    pub listado: String,
    /// Datos que siguen al programa después de una línea `→` (convención de
    /// los programas de prueba de Edimburgo).
    pub datos: Option<String>,
}

impl Imagen {
    /// Carga la imagen en la máquina y deja C en el arranque.
    pub fn cargar(&self, m: &mut Machine) {
        for &(a, v) in &self.cortos {
            m.store.set_short(a, v);
        }
        for (s, datos) in &self.tambor {
            m.drum.sector_mut(*s).copy_from_slice(datos);
        }
        m.c = self.arranque;
    }
}

pub type Error = ErrorCompilacion;

fn error(linea: usize, msg: impl Into<String>) -> Error {
    ErrorCompilacion { linea, msg: msg.into() }
}

struct Trozo {
    programa: u32,
    numero: u32,
    linea: usize,
    lineas: Vec<Linea>,
}

/// Supuesto: la cinta ya traducida comparte el tambor. Se conservan los
/// sectores convencionales de los capítulos sin programme, y se asignan
/// huecos a los capítulos de biblioteca y a los capítulos 0 para rmp.
fn asignar_capitulos(capitulos: &[Trozo]) -> Result<Vec<u32>, Error> {
    let mut ocupados: BTreeSet<_> = capitulos.iter()
        .filter(|c| c.programa == 0).map(|c| c.numero).collect();
    let mut primer_cero = true;
    let mut fisicos = Vec::with_capacity(capitulos.len());
    for c in capitulos {
        if c.programa == 0 && (c.numero != 0 || primer_cero) {
            fisicos.push(c.numero);
            if c.numero == 0 {
                primer_cero = false;
            }
        } else {
            let n = (1..=21).find(|n| !ocupados.contains(n))
                .ok_or_else(|| error(c.linea, "el tambor admite 22 capítulos traducidos en total"))?;
            ocupados.insert(n);
            fisicos.push(n);
        }
    }
    Ok(fisicos)
}

/// Compila un programa Autocode.
pub fn compilar(fuente: &str) -> Result<Imagen, Error> {
    // Datos después de una línea "→" o "->".
    let mut programa = String::new();
    let mut datos = None;
    let mut lineas_fuente = fuente.lines();
    for l in lineas_fuente.by_ref() {
        let t = l.split("//").next().unwrap_or("").trim();
        if t == "->" || t == "→" {
            // Los datos llegan hasta otra línea "->", si la hay.
            let es_fin = |l: &&str| matches!(l.split("//").next().unwrap_or("").trim(), "->" | "→");
            datos = Some(lineas_fuente.take_while(|l| !es_fin(l)).collect::<Vec<_>>().join("\n"));
            break;
        }
        programa.push_str(l);
        programa.push('\n');
    }
    let lineas = analizar(&programa).map_err(|e| error(e.linea, e.msg))?;

    // Partir en capítulos. Sin ninguna directiva chapter, todo es el capítulo 0.
    let hay_capitulos = lineas.iter().any(|l| matches!(l.sentencia, Some(Sentencia::Capitulo(_))));
    let mut capitulos: Vec<Trozo> = Vec::new();
    let mut salida_traduccion = String::new();
    let mut actual = (!hay_capitulos).then(|| Trozo { programa: 0, numero: 0, linea: 0, lineas: Vec::new() });
    let mut programa_actual = 0;
    let mut titulo = false;
    for l in lineas {
        if titulo {
            titulo = false;
            salida_traduccion.push_str(&l.texto);
            salida_traduccion.push('\n');
            continue;
        }
        match &l.sentencia {
            Some(Sentencia::Titulo) => titulo = true,
            Some(Sentencia::Programa(n)) => {
                if actual.is_some() {
                    return Err(error(l.numero, "falta close antes de programme"));
                }
                programa_actual = *n;
            }
            Some(Sentencia::Capitulo(n)) => {
                if actual.is_some() {
                    return Err(error(l.numero, "falta close antes de chapter"));
                }
                if *n > 21 {
                    return Err(error(l.numero, "el capítulo máximo es el 21"));
                }
                let programa = if *n == 0 { 0 } else { programa_actual };
                if *n != 0 && capitulos.iter().any(|c| c.programa == programa && c.numero == *n) {
                    return Err(error(l.numero, format!("capítulo {n} repetido en el programa {programa}")));
                }
                actual = Some(Trozo { programa, numero: *n, linea: l.numero, lineas: Vec::new() });
                if *n == 0 {
                    programa_actual = 0;
                }
            }
            Some(Sentencia::Close) => match actual.take() {
                Some(c) => capitulos.push(c),
                None if !hay_capitulos => {}
                None => return Err(error(l.numero, "close sin chapter")),
            },
            _ => match actual.as_mut() {
                Some(c) => c.lineas.push(l),
                None => return Err(error(l.numero, "instrucción fuera de un capítulo")),
            },
        }
    }
    if let Some(c) = actual {
        if hay_capitulos {
            return Err(error(c.linea, format!("falta close del capítulo {}", c.numero)));
        }
        capitulos.push(c);
    }
    if !capitulos.last().is_some_and(|c| c.numero == 0) {
        return Err(error(0, "el último capítulo debe ser el 0, que arranca el programa"));
    }
    let fisicos = asignar_capitulos(&capitulos)?;
    let numeros: BTreeSet<u32> = fisicos.iter().copied().collect();
    let ceros: Vec<_> = capitulos.iter().zip(&fisicos)
        .filter(|(c, _)| c.numero == 0).map(|(_, n)| *n).collect();
    let destinos: HashMap<_, _> = capitulos.iter().zip(&fisicos).filter(|(c, _)| c.numero != 0)
        .map(|(c, &n)| ((c.programa, c.numero), n)).collect();

    // Rutinas fijas.
    let pagina0 = runtime::pagina0().map_err(|e| error(0, format!("página 0: {e}")))?;
    let pagina1 = runtime::capitulos().map_err(|e| error(0, format!("página 1: {e}")))?;
    let entradas = Entradas { across: pagina1.labels[&1], down: pagina1.labels[&3], up: pagina1.labels[&4] };

    // Compilar cada capítulo; "variables N" toma la reserva de un capítulo anterior.
    let mut compilados = Vec::new();
    let mut reservas: HashMap<(u32, u32), Vec<(char, u16, u16)>> = HashMap::new();
    let mut ceros_pendientes = ceros.iter().peekable();
    for (pos, (c, &n)) in capitulos.iter().zip(&fisicos).enumerate() {
        let cero_actual = **ceros_pendientes.peek().unwrap();
        let mut previa = None;
        let mut resto = Vec::new();
        for l in &c.lineas {
            if let Some(Sentencia::VariablesDe(k)) = l.sentencia {
                let origen = if k != 0 {
                    (c.programa, k)
                } else if c.programa == 0 {
                    (0, 0)
                } else {
                    capitulos[..pos].iter().rev().find(|a| a.programa != c.programa)
                        .map_or((0, 0), |a| (a.programa, a.numero))
                };
                match reservas.get(&origen) {
                    Some(r) => previa = Some(r.clone()),
                    None => {
                        return Err(error(l.numero, format!("variables {k}: ese capítulo no está antes en la cinta")))
                    }
                }
            } else {
                let mut l = l.clone();
                let llamada = match l.sentencia {
                    Some(Sentencia::Across(m, k)) => Some((m, k, c.programa, false)),
                    Some(Sentencia::Down(m, k)) => Some((m, k, c.programa, true)),
                    Some(Sentencia::AcrossPrograma(m, k, p)) => Some((m, k, p, false)),
                    Some(Sentencia::DownPrograma(m, k, p)) => Some((m, k, p, true)),
                    _ => None,
                };
                if let Some((m, k, p, down)) = llamada {
                    let destino = if k == 0 && p == c.programa {
                        cero_actual
                    } else {
                        *destinos.get(&(p, k)).ok_or_else(|| error(l.numero, format!("no existe el capítulo {k} del programa {p}")))?
                    };
                    l.sentencia = Some(if down { Sentencia::Down(m, destino) } else { Sentencia::Across(m, destino) });
                }
                resto.push(l);
            }
        }
        let armado = compilar_capitulo(n, &resto, previa.as_deref(), &numeros, entradas)?;
        reservas.insert((c.programa, c.numero), armado.reserva.clone());
        compilados.push(armado);
        if c.numero == 0 {
            ceros_pendientes.next();
        }
    }
    let todas: HashMap<u32, HashMap<u32, u16>> = compilados
        .iter()
        .map(|c| {
            let t = c
                .etiquetas
                .iter()
                .filter(|(e, _)| **e < 1000)
                .map(|(e, p)| (*e, runtime::mapa::CAPITULO_INICIO + *p as u16))
                .collect();
            (c.numero, t)
        })
        .collect();

    let mut imagen = Imagen { datos, arranque: runtime::mapa::CAPITULO_INICIO, ..Default::default() };
    imagen.cortos.extend(pagina0.shorts.iter().copied());
    imagen.cortos.extend(pagina1.shorts.iter().copied());
    // Capítulo actual al arrancar: el 0.
    imagen.cortos.push((2 * pagina1.labels[&90] as usize, primer_sector(0)));
    let superposiciones: BTreeSet<_> = compilados.iter()
        .flat_map(|c| c.superposiciones.iter().copied()).collect();
    for rutina in superposiciones {
        let p = rutina.ensamblar().map_err(|e| error(0, format!("sector {}: {e}", rutina.sector())))?;
        let mut datos = vec![0; usize::from(rutina.paginas()) * 128];
        let inicio = usize::from(rutina.origen()) * 2;
        for (direccion, palabra) in p.shorts {
            let Some(celda) = direccion.checked_sub(inicio).and_then(|i| datos.get_mut(i)) else {
                return Err(error(0, format!("la rutina del sector {} excede sus páginas", rutina.sector())));
            };
            *celda = palabra;
        }
        if rutina == runtime::Superposicion::Rmp {
            let tabla = usize::from(p.labels[&80]) * 2 - inicio;
            for (i, &n) in ceros.iter().skip(1).enumerate() {
                datos[tabla + i] = primer_sector(n);
            }
        }
        for (i, pagina) in datos.chunks_exact(128).enumerate() {
            imagen.tambor.push((usize::from(rutina.sector()) + i, pagina.to_vec()));
        }
    }
    for (c, fuente) in compilados.iter().zip(&capitulos) {
        let a = armar_capitulo(c, &todas)?;
        let nombre = if fuente.programa == 0 { fuente.numero.to_string() }
            else { format!("{}/{}", fuente.programa, fuente.numero) };
        for p in &a.psa {
            salida_traduccion.push_str(&format!("{nombre} {p}\n"));
        }
        imagen.listado.push_str(&format!("capítulo {nombre} ({} registros libres)\n{}\n", a.libres, a.listado));
        let s0 = primer_sector(a.numero) as usize;
        for k in 0..13 {
            imagen.tambor.push((s0 + k, a.paginas[k * 128..(k + 1) * 128].to_vec()));
        }
        if a.numero == 0 {
            for (k, v) in a.paginas.iter().enumerate() {
                imagen.cortos.push((256 + k, *v));
            }
        }
    }
    imagen.salida_traduccion = salida_traduccion;
    Ok(imagen)
}
