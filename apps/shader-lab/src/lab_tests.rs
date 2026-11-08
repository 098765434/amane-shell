//! The kit's lab, run on this lab's own files: every effect loads, every
//! pass's shader is embedded, and the glass shader is the kit's.

use std::path::Path;

use emaki_kit::lab::config::Config;
use emaki_kit::lab::manifest::normal;
use emaki_kit::lab::{file, files, manifest};

/// The lab with these files registered, as `main` does.
fn registered() {
    emaki_kit::lab::register(crate::files::FILES);
}

#[test]
fn every_effect_folder_is_embedded_and_sound() {
    registered();
    let effects: Vec<&str> = files()
        .iter()
        .filter_map(|(path, _)| path.strip_prefix("effects/")?.strip_suffix("/effect.json"))
        .collect();

    assert!(effects.contains(&"chrome"), "{effects:?}");
    assert!(effects.contains(&"liquid-glass"), "{effects:?}");

    for effect in effects {
        let manifest = manifest(effect).unwrap_or_else(|| panic!("{effect} won't load"));

        for pass in &manifest.passes {
            let path = normal(&Path::new("effects").join(effect).join(&pass.shader));
            let name = path.to_string_lossy().replace('\\', "/");

            assert!(file(&name).is_some(), "{effect}: {name} isn't embedded");
        }

        for preset in &manifest.presets {
            for key in preset.values.keys() {
                assert!(
                    manifest.params.iter().any(|param| &param.key == key),
                    "{effect}: {} sets {key}, which isn't a param",
                    preset.name
                );
            }
        }
    }
}

#[test]
fn the_glass_shader_is_the_kit_s_own() {
    registered();
    assert_eq!(
        file("effects/glass/glass.wgsl"),
        Some(include_str!("../../../crates/kit/src/look/glass.wgsl"))
    );
}

#[test]
fn a_variant_takes_what_it_leaves_out_from_what_it_extends() {
    registered();
    let manifest = manifest("chrome-wallpaper").unwrap();

    assert_eq!(manifest.name, "Reflective chrome (wallpaper)");
    assert_eq!(manifest.params[0].key, "reflect");
    assert_eq!(manifest.params.last().unwrap().key, "evolution");
    assert!(manifest.gradient.is_some());
}

#[test]
fn a_config_packs_its_own_gradient_and_values() {
    registered();
    let manifest = manifest("chrome").unwrap();

    let mut config = Config::new("chrome");
    config.values.insert("heightIntensity".into(), 150.0);
    config.gradient.stops = Some(vec![[0.0, 1.0, 0.0, 0.0, 1.0], [1.0, 0.0, 0.0, 1.0, 1.0]]);

    let rows = config.rows(&manifest);

    // depth, then the two stops and their count
    assert_eq!(rows[11][1], 150.0);
    assert_eq!(rows[1], [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(rows[2], [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(rows[11][0], 2.0);
}
