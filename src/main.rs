use core::f32;
use core::f64;
use deuxfleurs::egui::Widget;
use deuxfleurs::load_mesh;
use deuxfleurs::types::SurfaceIndices;
use deuxfleurs::ui::LoadObjButton;
use deuxfleurs::{RunningState, egui};
use uvat_rs::utils::{build_edge_map, compute_tutte_parameterization, get_boundary_loop};
use uvat_rs::{UVAT, UVATOptions};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

fn main() {
    pollster::block_on(run());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    // Temp fix for performance
    faer::set_global_parallelism(faer::Par::Rayon(core::num::NonZero::new(1).unwrap()));
    let mut options = UVATOptions::default();
    let mut solver: Option<UVAT<u32>> = None;
    let mut v_values = Vec::new();
    let mut p = Vec::new();
    let mut running = false;
    let mut ran_once = false;

    let callback = move |ui: &mut egui::Ui, state: &mut RunningState| {
        if running {
            //let mut v_values = v_values.borrow_mut();
            let mut surface = state.get_surface_mut("Surface").unwrap();
            let f = &surface.geometry().indices;
            let f = match f {
                SurfaceIndices::Triangles(f) => f.clone(),
                _ => panic!(),
            };
            running = !solver
                .as_mut()
                .unwrap()
                .single_step(&f, &mut p, &mut v_values);

            // Convert to f32 and corner param for displaying
            let p: Vec<_> = p
                .iter()
                .map(|row| [row[0] as f32, row[1] as f32, 0.])
                .collect();
            let new_uv: Vec<_> = f
                .as_flattened()
                .iter()
                .map(|&i| [p[i as usize][0], p[i as usize][1]])
                .collect();
            surface.add_corner_uv_map("UV map", new_uv);
            surface.add_face_scalar("V", &*v_values);
            surface.set_data(Some("V"));
            let mut param = state.register_surface("UVAT parameterization", p, f);
            param.show_edges(true);
            param.add_face_scalar("V", &*v_values);
            param.set_data(Some("V"));
        }

        ui.label("Load a triangular mesh homeomorphic to a disk or of genus 0, then run UVAT (this may take a while to compute).");
        let response = LoadObjButton::new("Load mesh", "Surface", state).ui(ui);
        if response.clicked() {
            ran_once = false;
        }
        ui.add(
            egui::Slider::new(&mut options.lambda, 0.0..=10.0)
                .text("lambda")
                .clamping(egui::SliderClamping::Never),
        );
        ui.add(
            egui::Slider::new(&mut options.epsilon_start, 0.1..=1.0)
                .text("epsilon 1")
                .clamping(egui::SliderClamping::Never),
        );
        ui.add(
            egui::Slider::new(&mut options.epsilon_end, 0.01..=0.1)
                .text("epsilon 2")
                .clamping(egui::SliderClamping::Never),
        );
        if ui.add(egui::Button::new("UVAT")).clicked() {
            if let Some(mut surface) = state.get_surface_mut("Surface") {
                let mut v: Vec<_> = surface
                    .geometry()
                    .vertices
                    .iter()
                    .map(|row| [row[0] as f64, row[1] as f64, row[2] as f64])
                    .collect();
                let f = &surface.geometry().indices;
                let mut f = match f {
                    SurfaceIndices::Triangles(f) => f.clone(),
                    _ => panic!(),
                };

                let mut e = build_edge_map(&f, v.len());
                let mut b = get_boundary_loop(&f, &e);
                // If no boundary is found, we assume genus 0 surface and apply a simple cut
                if b.len() == 0 {
                    let v0 = v[f[0][1] as usize].to_owned();
                    v.push(v0);
                    f[0][1] = v.len() as u32 - 1;
                    e = build_edge_map(&f, v.len());
                    b = get_boundary_loop(&f, &e);
                }

                let tutte = compute_tutte_parameterization(&v, &f, e, &b[0]);
                let verts_c: Vec<_> = f
                    .as_flattened()
                    .iter()
                    .map(|&i| [tutte[i as usize][0] as f32, tutte[i as usize][1] as f32])
                    .collect();
                surface.add_corner_uv_map("Tutte parameterization", verts_c);

                p = tutte;
                //*v_values.borrow_mut() = vec![1.; f.len()];
                v_values = vec![1.; f.len()];
                solver = Some(UVAT::new(&v, &f, &mut p, options.clone()));
                running = true;
                ran_once = true;
            }
        }

        if running {
            ui.label("Running...");
        } else if ran_once {
            ui.label("Done!");
        }
    };

    let url_path = option_env!("URL_PATH").unwrap_or(".");
    let mesh_path = std::path::Path::new(url_path).join("./assets/camelhead.obj");
    let mesh_str = mesh_path.to_str().unwrap();
    let mut handle = deuxfleurs::init().with_callback(callback);
    let (v, f) = load_mesh(mesh_str).await.unwrap();
    handle.register_surface("Surface", v, f);
    handle.run(1080, 720, Some("deuxfleurs"));
}
