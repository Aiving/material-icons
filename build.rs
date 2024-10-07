use std::{
    collections::{hash_map::Entry, HashMap},
    env, io,
    path::{Path, PathBuf},
};

use codegen::{Function, Scope, Variant};
use serde::Deserialize;

const CONFIG_FILE: &str = "icons.json";
const SHIPPED_ICONS_PATH: &str = "icons";

const CONSTANTS_FILE: &str = "icons.rs";

#[derive(Debug, Default, Deserialize)]
enum IconStyle {
    #[default]
    #[serde(rename = "outlined")]
    Outlined,
    #[serde(rename = "rounded")]
    Rounded,
    #[serde(rename = "sharp")]
    Sharp,
}

#[derive(Deserialize)]
struct IconInfo {
    name: String,
    #[serde(default)]
    style: IconStyle,
    #[serde(default)]
    filled: bool,
}

impl From<String> for IconInfo {
    fn from(name: String) -> Self {
        Self {
            name,
            style: IconStyle::default(),
            filled: false,
        }
    }
}

impl From<Icon> for IconInfo {
    fn from(value: Icon) -> Self {
        match value {
            Icon::Simple(name) => name.into(),
            Icon::Configured(icon_info) => icon_info,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Icon {
    Configured(IconInfo),
    Simple(String),
}

fn load_icons(dir: &str) -> Result<Vec<IconInfo>, io::Error> {
    let config_path: PathBuf = [dir, CONFIG_FILE].iter().collect();
    let config_file = std::fs::read_to_string(config_path)?;

    let icons: Vec<Icon> =
        serde_json::from_str(&config_file).expect("Couldn't parse icon config file");

    Ok(icons.into_iter().map(Into::into).collect())
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let mut manifest_dir = Path::new(&out_dir).canonicalize().unwrap();

    eprintln!("Canonical manifest dir: {manifest_dir:?}");

    let (config, config_dir) = if cfg!(docsrs) {
        if let Ok(source_dir) = env::var("SOURCE_DIR") {
            (load_icons(&source_dir).unwrap_or_default(), source_dir)
        } else {
            (Vec::new(), "".into())
        }
    } else {
        // Try finding the target directory which is just below the manifest directory
        // of the user.
        // Unfortunately, the CARGO_MANIFEST_DIR env var passed by cargo always points
        // to this crate, so we wouldn't find the users config file this way.
        while !manifest_dir.join("Cargo.toml").exists() {
            if !manifest_dir.pop() {
                panic!("Couldn't find your manifest directory");
            }
        }

        let config_dir = manifest_dir
            .to_str()
            .expect("Couldn't convert manifest directory to string")
            .to_owned();
        (
            load_icons(&config_dir).expect("Couldn't find `icons.json` next to `Cargo.toml`"),
            config_dir,
        )
    };

    eprintln!("Canonical config dir: {config_dir:?}");

    println!("cargo:rerun-if-changed={config_dir}/icons.json");

    let mut icons: HashMap<String, Vec<(IconInfo, PathBuf)>> = HashMap::new();

    for icon in config {
        let file_name = format!(
            "{}{}.svg",
            if icon.filled { "filled-" } else { "" },
            match icon.style {
                IconStyle::Outlined => "outlined",
                IconStyle::Rounded => "rounded",
                IconStyle::Sharp => "sharp",
            },
        );

        let path = PathBuf::from(SHIPPED_ICONS_PATH)
            .join(&icon.name)
            .join(file_name);

        if path.exists() {
            match icons.entry(icon.name.clone()) {
                Entry::Occupied(mut entry) => entry.get_mut().push((icon, path)),
                Entry::Vacant(entry) => {
                    entry.insert(vec![(icon, path)]);
                }
            }
        } else {
            panic!("Icon {} not found at {}", icon.name, path.display());
        }
    }

    let mut root = Scope::new();

    root.new_enum("IconStyle")
        .vis("pub")
        .push_variant(Variant::new("Outlined"))
        .push_variant(Variant::new("Rounded"))
        .push_variant(Variant::new("Sharp"));

    let mut name_variants = Vec::new();

    for (name, variants) in icons {
        let mut match_variants = Vec::new();

        for (info, path) in variants {
            let const_name = format!(
                "ICON_{}_{}{}",
                name.to_uppercase(),
                if info.filled { "FILLED_" } else { "" },
                match info.style {
                    IconStyle::Outlined => "OUTLINED",
                    IconStyle::Rounded => "ROUNDED",
                    IconStyle::Sharp => "SHARP",
                }
            );

            root.raw(format!(
                "const {const_name}: &[u8] = include_bytes!(\"{}\");",
                path.canonicalize().unwrap().display()
            ));

            match_variants.push(format!(
                "(IconStyle::{:?}, {}) => {const_name},",
                info.style, info.filled
            ));
        }

        let mut func = Function::new(format!("icon_{name}"));

        func.vis("pub")
            .arg("style", "IconStyle")
            .arg("filled", "bool")
            .ret("&'static [u8]")
            .line(format!(
                "match (style, filled) {{
    {}
    _ => panic!(\"there is no such icon\")
}}",
                match_variants.join("\n")
            ));

        root.push_fn(func);

        name_variants.push(format!("{name:?} => icon_{name}(style, filled),"));
    }

    root.new_fn("icon")
        .vis("pub")
        .arg("name", "impl AsRef<str>")
        .arg("style", "IconStyle")
        .arg("filled", "bool")
        .ret("&'static [u8]")
        .line(format!(
            "match name.as_ref() {{
    {}
    value => panic!(\"there is no icon called {{value}}\")
}}",
            name_variants.join("\n")
        ));

    std::fs::write(Path::new(&out_dir).join(CONSTANTS_FILE), root.to_string()).unwrap();
}
